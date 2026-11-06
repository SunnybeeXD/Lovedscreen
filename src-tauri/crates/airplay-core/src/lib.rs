mod ffi;
mod sink;
pub use sink::{AudioCodec, MediaSink, SharedSink};
use std::sync::Arc;

pub struct Server {
    engine: ffi::Engine,
}

impl Server {
    pub fn start(name: &str, sink: Arc<dyn MediaSink>) -> Result<Server, String> {
        Server::start_with_fps(name, sink, 30)
    }

    pub fn start_with_fps(name: &str, sink: Arc<dyn MediaSink>, max_fps: u8) -> Result<Server, String> {
        Ok(Server {
            engine: ffi::Engine::start(name, sink, max_fps)?,
        })
    }

    pub fn port(&self) -> u16 {
        self.engine.port()
    }
}
