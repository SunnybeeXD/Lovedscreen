use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

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
    });
}

struct Recording {
    video: PathBuf,
    audio: PathBuf,
    out: PathBuf,
}

fn recording_paths() -> Result<Recording, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|e| e.to_string())?;
    let dir = PathBuf::from(home).join("Videos").join("Lovedscreen");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    Ok(Recording {
        video: dir.join(format!("{ts}.video.mkv")),
        audio: dir.join(format!("{ts}.audio.mkv")),
        out: dir.join(format!("{ts}.mp4")),
    })
}

fn gst_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn video_sink(r: &Recording) -> String {
    format!(
        "tee name=t ! queue ! autovideosink t. ! queue ! videoconvert ! x264enc tune=zerolatency speed-preset=veryfast ! h264parse ! matroskamux ! filesink location=\"{}\"",
        gst_path(&r.video)
    )
}

fn audio_sink(r: &Recording) -> String {
    format!(
        "tee name=a ! queue ! autoaudiosink a. ! queue ! audioconvert ! avenc_aac ! matroskamux ! filesink location=\"{}\"",
        gst_path(&r.audio)
    )
}

fn mux(r: &Recording) -> String {
    let inputs: Vec<&PathBuf> = [&r.video, &r.audio]
        .into_iter()
        .filter(|p| p.metadata().map(|m| m.len() > 0).unwrap_or(false))
        .collect();
    if inputs.is_empty() {
        return String::new();
    }
    let ffmpeg = std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".into());
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y");
    for i in &inputs {
        cmd.arg("-i").arg(i);
    }
    cmd.args(["-c", "copy"]).arg(&r.out);
    match cmd.status() {
        Ok(s) if s.success() => {
            for i in inputs {
                let _ = std::fs::remove_file(i);
            }
            r.out.to_string_lossy().into_owned()
        }
        _ => format!("mux failed, raw files kept next to {}", r.out.display()),
    }
}

#[tauri::command]
fn start_receiver(
    app: AppHandle,
    state: State<Receiver>,
    name: String,
    record: bool,
) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("receiver already running".into());
    }
    let rec = if record { Some(recording_paths()?) } else { None };
    let mut cmd = Command::new(receiver_path());
    cmd.args(["-n", &name]);
    if let Some(r) = &rec {
        cmd.args(["-vs", &video_sink(r), "-as", &audio_sink(r)]);
    }
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start receiver: {e}"))?;
    let out = child.stdout.take();
    if let Some(err) = child.stderr.take() {
        pipe_lines(app.clone(), err);
    }
    *slot = Some(child);
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Some(out) = out {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = handle.emit("receiver-line", line);
            }
        }
        let child = handle
            .state::<Receiver>()
            .0
            .lock()
            .ok()
            .and_then(|mut s| s.take());
        if let Some(mut c) = child {
            let _ = c.wait();
        }
        let saved = rec.as_ref().map(mux).unwrap_or_default();
        let _ = handle.emit("receiver-exit", saved);
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
        .invoke_handler(tauri::generate_handler![
            bonjour_status,
            start_receiver,
            stop_receiver
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
