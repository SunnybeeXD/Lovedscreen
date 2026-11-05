use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
struct Receiver(Mutex<Option<Child>>);

#[tauri::command]
fn bonjour_status() -> &'static str {
    #[cfg(windows)]
    {
        let out = match Command::new("sc").args(["query", "Bonjour Service"]).output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return "missing",
        };
        if out.contains("RUNNING") {
            "running"
        } else if out.contains("STOPPED") {
            "stopped"
        } else {
            "missing"
        }
    }
    #[cfg(not(windows))]
    {
        "running"
    }
}

fn receiver_path() -> String {
    std::env::var("UXPLAY_PATH").unwrap_or_else(|_| "uxplay".into())
}

fn pipe_lines<R: std::io::Read + Send + 'static>(app: AppHandle, stream: R) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let _ = app.emit("receiver-line", line);
        }
        let _ = app.emit("receiver-exit", ());
    });
}

#[tauri::command]
fn start_receiver(app: AppHandle, state: State<Receiver>, name: String) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("receiver already running".into());
    }
    let mut child = Command::new(receiver_path())
        .args(["-n", &name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start receiver: {e}"))?;
    if let Some(out) = child.stdout.take() {
        pipe_lines(app.clone(), out);
    }
    if let Some(err) = child.stderr.take() {
        pipe_lines(app, err);
    }
    *slot = Some(child);
    Ok(())
}

#[tauri::command]
fn stop_receiver(state: State<Receiver>) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = slot.take() {
        child.kill().map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Receiver::default())
        .invoke_handler(tauri::generate_handler![
            bonjour_status,
            start_receiver,
            stop_receiver
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
