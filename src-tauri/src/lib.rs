use airplay_core::{AudioCodec, MediaSink, Server};
use airplay_render::GstSink;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct Receiver(Mutex<Option<Server>>);

fn log_file_path() -> PathBuf {
    std::env::temp_dir().join("lovedscreen.log")
}

fn gst_log_path() -> PathBuf {
    std::env::temp_dir().join("lovedscreen-gst.log")
}

struct UiSink {
    app: AppHandle,
    inner: GstSink,
    file: Mutex<Option<File>>,
    start: Instant,
    video_frames: AtomicU64,
    audio_packets: AtomicU64,
}

impl UiSink {
    fn new(app: AppHandle, inner: GstSink) -> UiSink {
        UiSink {
            app,
            inner,
            file: Mutex::new(File::create(log_file_path()).ok()),
            start: Instant::now(),
            video_frames: AtomicU64::new(0),
            audio_packets: AtomicU64::new(0),
        }
    }

    fn note(&self, msg: &str) {
        let line = format!("{:9.3} {}", self.start.elapsed().as_secs_f64(), msg.trim_end());
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
        let _ = self.app.emit("receiver-line", line);
    }
}

impl MediaSink for UiSink {
    fn video_codec(&self, width: u16, height: u16, h265: bool) {
        self.note(&format!("video codec {width}x{height} h265={h265}"));
        self.inner.video_codec(width, height, h265);
        self.note("video codec handled");
    }
    fn video(&self, annexb: &[u8], pts_ns: u64) {
        let n = self.video_frames.fetch_add(1, Ordering::Relaxed) + 1;
        if n == 1 || n % 300 == 0 {
            self.note(&format!("video frames received: {n} (last {} bytes)", annexb.len()));
        }
        self.inner.video(annexb, pts_ns);
    }
    fn audio(&self, codec: AudioCodec, data: &[u8], pts_ns: u64) {
        let n = self.audio_packets.fetch_add(1, Ordering::Relaxed) + 1;
        if n == 1 || n % 500 == 0 {
            self.note(&format!("audio packets received: {n} codec={codec:?} (last {} bytes)", data.len()));
        }
        self.inner.audio(codec, data, pts_ns);
    }
    fn flush(&self) {
        self.note("flush");
        self.inner.flush();
    }
    fn connection(&self, open: bool) {
        self.note(&format!("connection open={open}"));
        self.inner.connection(open);
        let _ = self.app.emit("receiver-connection", open);
    }
    fn log(&self, level: i32, msg: &str) {
        self.note(&format!("[{level}] {msg}"));
    }
}

fn point_gstreamer_at_bundle(app: &AppHandle) {
    if std::env::var_os("GST_DEBUG").is_none() {
        std::env::set_var("GST_DEBUG", "2");
    }
    std::env::set_var("GST_DEBUG_NO_COLOR", "1");
    std::env::set_var("GST_DEBUG_FILE", gst_log_path());
    if let Ok(dir) = app.path().resource_dir() {
        let plugins = dir.join("gstreamer-1.0");
        if plugins.is_dir() {
            std::env::set_var("GST_PLUGIN_PATH_1_0", &plugins);
            std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &plugins);
            std::env::set_var(
                "GST_REGISTRY_1_0",
                std::env::temp_dir().join("lovedscreen-gst-registry.bin"),
            );
        }
    }
}

#[tauri::command]
async fn start_receiver(
    app: AppHandle,
    state: State<'_, Receiver>,
    name: String,
    fps: u8,
) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("receiver already running".into());
    }
    point_gstreamer_at_bundle(&app);
    let inner = GstSink::new()?;
    let sink = Arc::new(UiSink::new(app.clone(), inner));
    let weak = Arc::downgrade(&sink);
    let weak_logger = weak.clone();
    sink.inner.set_logger(Arc::new(move |m: &str| {
        if let Some(s) = weak_logger.upgrade() {
            s.note(m);
        }
    }));
    std::panic::set_hook(Box::new(move |info| {
        if let Some(s) = weak.upgrade() {
            s.note(&format!("PANIC: {info}"));
        }
    }));
    sink.note(&format!("starting receiver name={name:?}, log file {}", log_file_path().display()));
    let server = Server::start_with_fps(&name, sink.clone(), if fps >= 60 { 60 } else { 30 })?;
    sink.note(&format!("listening on port {}", server.port()));
    *slot = Some(server);
    Ok(())
}

#[tauri::command]
async fn stop_receiver(app: AppHandle, state: State<'_, Receiver>) -> Result<(), String> {
    let server = state.0.lock().map_err(|e| e.to_string())?.take();
    drop(server);
    let _ = app.emit("receiver-exit", ());
    Ok(())
}

#[tauri::command]
fn read_log() -> String {
    let tail = |p: PathBuf| {
        let s = String::from_utf8_lossy(&std::fs::read(&p).unwrap_or_default()).into_owned();
        let keep = s.len().saturating_sub(200_000);
        let from = s.char_indices().map(|(i, _)| i).find(|&i| i >= keep).unwrap_or(s.len());
        s[from..].to_string()
    };
    format!("{}\n--- gstreamer ---\n{}", tail(log_file_path()), tail(gst_log_path()))
}

pub fn run() {
    tauri::Builder::default()
        .manage(Receiver::default())
        .invoke_handler(tauri::generate_handler![start_receiver, stop_receiver, read_log])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
