use airplay_core::{AudioCodec, MediaSink, Server};
use airplay_render::GstSink;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct Receiver(Mutex<Option<Server>>);

struct UiSink {
    app: AppHandle,
    inner: GstSink,
}

impl MediaSink for UiSink {
    fn video_codec(&self, width: u16, height: u16, h265: bool) {
        self.inner.video_codec(width, height, h265);
    }
    fn video(&self, annexb: &[u8], pts_ns: u64) {
        self.inner.video(annexb, pts_ns);
    }
    fn audio(&self, codec: AudioCodec, data: &[u8], pts_ns: u64) {
        self.inner.audio(codec, data, pts_ns);
    }
    fn flush(&self) {
        self.inner.flush();
    }
    fn connection(&self, open: bool) {
        self.inner.connection(open);
        let _ = self.app.emit("receiver-connection", open);
    }
    fn log(&self, _level: i32, msg: &str) {
        let _ = self.app.emit("receiver-line", msg.to_string());
    }
}

fn point_gstreamer_at_bundle(app: &AppHandle) {
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
) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("receiver already running".into());
    }
    point_gstreamer_at_bundle(&app);
    let inner = GstSink::new()?;
    let sink = Arc::new(UiSink {
        app: app.clone(),
        inner,
    });
    *slot = Some(Server::start(&name, sink)?);
    Ok(())
}

#[tauri::command]
async fn stop_receiver(app: AppHandle, state: State<'_, Receiver>) -> Result<(), String> {
    let server = state.0.lock().map_err(|e| e.to_string())?.take();
    drop(server);
    let _ = app.emit("receiver-exit", ());
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Receiver::default())
        .invoke_handler(tauri::generate_handler![start_receiver, stop_receiver])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
