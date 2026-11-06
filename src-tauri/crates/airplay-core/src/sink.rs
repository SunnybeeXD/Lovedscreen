use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCodec {
    Alac,
    AacEld,
    Aac,
}

pub trait MediaSink: Send + Sync + 'static {
    fn video_codec(&self, width: u16, height: u16, h265: bool);
    fn video(&self, annexb: &[u8], pts_ns: u64);
    fn audio(&self, codec: AudioCodec, data: &[u8], pts_ns: u64);
    fn flush(&self);
    fn connection(&self, open: bool);
    fn log(&self, level: i32, msg: &str);
}

pub type SharedSink = Arc<dyn MediaSink>;
