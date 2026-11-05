use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct Receiver(Mutex<Option<Child>>);

fn receiver_command(app: &AppHandle) -> Command {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("uxplay"))
        .filter(|d| d.join("uxplay.exe").exists());
    let mut cmd = match bundled {
        Some(dir) => {
            let plugins = dir.join("gstreamer-1.0");
            let mut c = Command::new(dir.join("uxplay.exe"));
            c.env("GST_PLUGIN_PATH_1_0", &plugins)
                .env("GST_PLUGIN_SYSTEM_PATH_1_0", &plugins)
                .env(
                    "GST_REGISTRY_1_0",
                    std::env::temp_dir().join("lovedscreen-gst-registry.bin"),
                );
            c
        }
        None => Command::new(std::env::var("UXPLAY_PATH").unwrap_or_else(|_| "uxplay".into())),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

fn pipe_lines<R: std::io::Read + Send + 'static>(app: AppHandle, stream: R) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let _ = app.emit("receiver-line", line);
        }
    });
}

#[tauri::command]
fn start_receiver(app: AppHandle, state: State<Receiver>, name: String) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("receiver already running".into());
    }
    let mut child = receiver_command(&app)
        .args(["-n", &name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start receiver: {e}"))?;
    let out = child.stdout.take();
    if let Some(err) = child.stderr.take() {
        pipe_lines(app.clone(), err);
    }
    *slot = Some(child);
    std::thread::spawn(move || {
        if let Some(out) = out {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = app.emit("receiver-line", line);
            }
        }
        let child = app
            .state::<Receiver>()
            .0
            .lock()
            .ok()
            .and_then(|mut s| s.take());
        if let Some(mut c) = child {
            let _ = c.wait();
        }
        let _ = app.emit("receiver-exit", ());
    });
    Ok(())
}

#[tauri::command]
fn stop_receiver(state: State<Receiver>) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(child) = slot.as_mut() {
        child.kill().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Receiver::default())
        .invoke_handler(tauri::generate_handler![start_receiver, stop_receiver])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
