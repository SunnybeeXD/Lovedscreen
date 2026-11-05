mod ffi;
mod sink;
pub use sink::{AudioCodec, MediaSink, SharedSink};
use std::sync::Arc;

pub struct Server {
    engine: ffi::Engine,
}

impl Server {
    pub fn start(name: &str, sink: Arc<dyn MediaSink>) -> Result<Server, String> {
        Ok(Server {
            engine: ffi::Engine::start(name, sink)?,
        })
    }

    pub fn port(&self) -> u16 {
        self.engine.port()
    }
}
