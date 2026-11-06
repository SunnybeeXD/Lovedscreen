use airplay_core::{AudioCodec, MediaSink};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};

const LATENCY_NS: u64 = 250_000_000;

const ALAC_CAPS: &str = concat!(
    "audio/x-alac,mpegversion=(int)4,channels=(int)2,rate=(int)44100,stream-format=raw,codec_data=(buffer)",
    "00000024", "616c6163", "00000000", "00000160", "0010280a", "0e0200ff", "00000000", "00000000", "0000ac44"
);
const AAC_LC_CAPS: &str = "audio/mpeg,mpegversion=(int)4,channels=(int)2,rate=(int)44100,stream-format=raw,codec_data=(buffer)1210";
const AAC_ELD_CAPS: &str = "audio/mpeg,mpegversion=(int)4,channels=(int)2,rate=(int)44100,stream-format=raw,codec_data=(buffer)f8e85000";

struct Pipe {
    pipeline: gst::Pipeline,
    src: gst_app::AppSrc,
}

impl Pipe {
    fn stop(&self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }

    fn flush(&self) {
        self.src.send_event(gst::event::FlushStart::new());
        self.src.send_event(gst::event::FlushStop::new(false));
    }
}

struct Inner {
    video: Option<Pipe>,
    h265: bool,
    audio: Option<(AudioCodec, Pipe)>,
    base: Option<gst::ClockTime>,
    offset: Option<u64>,
}

impl Inner {
    fn reset(&mut self) {
        if let Some(p) = self.video.take() {
            p.stop();
        }
        if let Some((_, p)) = self.audio.take() {
            p.stop();
        }
        self.base = None;
        self.offset = None;
    }
}

type Logger = Arc<dyn Fn(&str) + Send + Sync>;

pub struct GstSink {
    video_sink: String,
    audio_sink: String,
    software: bool,
    clock: gst::Clock,
    logger: Mutex<Option<Logger>>,
    inner: Mutex<Inner>,
}

fn software_mode() -> bool {
    std::env::var("LOVEDSCREEN_SOFTWARE").map(|v| v == "1").unwrap_or(false)
}

fn default_video_sink() -> String {
    if let Ok(v) = std::env::var("LOVEDSCREEN_VIDEO_SINK") {
        if !v.is_empty() {
            return v;
        }
    }
    if cfg!(windows) { "d3d11videosink" } else { "autovideosink" }.to_string()
}

fn prefer_hardware_decoders() {
    for name in ["d3d11h264dec", "d3d11h265dec"] {
        if let Some(f) = gst::ElementFactory::find(name) {
            f.set_rank(gst::Rank::PRIMARY + 1);
        }
    }
}

fn find_fullscreen_element(sink: &gst::Bin) -> Option<gst::Element> {
    sink.iterate_recurse()
        .into_iter()
        .flatten()
        .find(|e| e.find_property("fullscreen").is_some())
}

fn is_fullscreen_key(s: &gst::StructureRef) -> bool {
    if s.name() != "application/x-gst-navigation" || s.get::<&str>("event").ok() != Some("key-press") {
        return false;
    }
    let key = s.get::<&str>("key").unwrap_or("");
    if key == "F11" {
        return true;
    }
    let state = s.value("state").map(|v| format!("{:?}", v).to_lowercase()).unwrap_or_default();
    (key == "Return" || key == "KP_Enter") && (state.contains("alt") || state.contains("mod1"))
}

impl GstSink {
    pub fn new() -> Result<GstSink, String> {
        GstSink::with_sinks(&default_video_sink(), "autoaudiosink")
    }

    pub fn with_sinks(video: &str, audio: &str) -> Result<GstSink, String> {
        gst::init().map_err(|e| e.to_string())?;
        let software = software_mode();
        if !software {
            prefer_hardware_decoders();
        }
        Ok(GstSink {
            video_sink: video.to_string(),
            audio_sink: audio.to_string(),
            software,
            clock: gst::SystemClock::obtain(),
            logger: Mutex::new(None),
            inner: Mutex::new(Inner { video: None, h265: false, audio: None, base: None, offset: None }),
        })
    }

    pub fn set_logger(&self, f: Arc<dyn Fn(&str) + Send + Sync>) {
        *self.logger.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    fn note(&self, msg: &str) {
        let logger = self.logger.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match logger {
            Some(f) => f(msg),
            None => eprintln!("airplay-render: {msg}"),
        }
    }

    fn watch_bus(&self, pipeline: &gst::Pipeline, tag: &'static str) {
        let Some(bus) = pipeline.bus() else { return };
        let weak = pipeline.downgrade();
        let logger = self.logger.lock().unwrap_or_else(|e| e.into_inner()).clone();
        std::thread::spawn(move || {
            while weak.upgrade().is_some() {
                let msg = bus.timed_pop_filtered(
                    gst::ClockTime::from_mseconds(250),
                    &[gst::MessageType::Error, gst::MessageType::Warning],
                );
                if let (Some(m), Some(f)) = (msg, logger.as_ref()) {
                    match m.view() {
                        gst::MessageView::Error(e) => f(&format!("{tag} ERROR: {} ({:?})", e.error(), e.debug())),
                        gst::MessageView::Warning(w) => f(&format!("{tag} warning: {} ({:?})", w.error(), w.debug())),
                        _ => {}
                    }
                }
            }
        });
    }

    #[doc(hidden)]
    pub fn pipelines(&self) -> (Option<gst::Pipeline>, Option<gst::Pipeline>) {
        let g = self.lock();
        (g.video.as_ref().map(|p| p.pipeline.clone()), g.audio.as_ref().map(|(_, p)| p.pipeline.clone()))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn base(&self, g: &mut Inner) -> gst::ClockTime {
        *g.base.get_or_insert_with(|| self.clock.time().unwrap_or(gst::ClockTime::ZERO))
    }

    fn timestamp(&self, g: &mut Inner, pts_ns: u64) -> gst::ClockTime {
        let base = self.base(g);
        let now = self.clock.time().unwrap_or(base).nseconds().saturating_sub(base.nseconds());
        let offset = *g.offset.get_or_insert_with(|| (now + LATENCY_NS).wrapping_sub(pts_ns));
        gst::ClockTime::from_nseconds(pts_ns.wrapping_add(offset))
    }

    fn start(&self, g: &mut Inner, head: &str, caps: &str, sink: &str, navigation: bool, tag: &'static str) -> Result<Pipe, String> {
        let base = self.base(g);
        self.note(&format!("{tag}: building pipeline: appsrc ! {head} ! {sink}"));
        let pipeline = gst::parse::launch(&format!("appsrc name=src ! {head}"))
            .map_err(|e| e.to_string())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "not a pipeline".to_string())?;
        self.note(&format!("{tag}: head created"));
        let tail = pipeline.by_name("tail").ok_or("missing tail")?;
        let bin = gst::parse::bin_from_description(sink, true).map_err(|e| e.to_string())?;
        self.note(&format!("{tag}: sink created"));
        pipeline.add(&bin).map_err(|e| e.to_string())?;
        tail.link(&bin).map_err(|e| e.to_string())?;
        self.note(&format!("{tag}: linked"));
        self.watch_bus(&pipeline, tag);
        if let Some(el) = find_fullscreen_element(&bin) {
            if el.find_property("fullscreen-toggle-mode").is_some() {
                el.set_property_from_str("fullscreen-toggle-mode", "alt-enter");
            }
        }
        if navigation {
            if let Some(pad) = bin.static_pad("sink") {
                let weak = bin.downgrade();
                pad.add_probe(gst::PadProbeType::EVENT_UPSTREAM, move |_, info| {
                    if let Some(gst::PadProbeData::Event(ev)) = &info.data {
                        if ev.structure().map(is_fullscreen_key).unwrap_or(false) {
                            if let Some(el) = weak.upgrade().and_then(|b| find_fullscreen_element(&b)) {
                                let on: bool = el.property("fullscreen");
                                el.set_property("fullscreen", !on);
                            }
                        }
                    }
                    gst::PadProbeReturn::Ok
                });
            }
        }
        let src = pipeline
            .by_name("src")
            .and_then(|e| e.downcast::<gst_app::AppSrc>().ok())
            .ok_or("missing appsrc")?;
        src.set_caps(Some(&gst::Caps::from_str(caps).map_err(|e| e.to_string())?));
        src.set_is_live(true);
        src.set_format(gst::Format::Time);
        pipeline.use_clock(Some(&self.clock));
        pipeline.set_start_time(gst::ClockTime::NONE);
        pipeline.set_base_time(base);
        self.note(&format!("{tag}: setting state to playing"));
        pipeline.set_state(gst::State::Playing).map_err(|e| e.to_string())?;
        self.note(&format!("{tag}: playing"));
        Ok(Pipe { pipeline, src })
    }

    fn start_video(&self, g: &mut Inner) -> Result<(), String> {
        if let Some(p) = g.video.take() {
            p.stop();
        }
        let convert = if self.video_sink.starts_with("d3d11") && !self.software { "d3d11convert" } else { "videoconvert" };
        let (parse, caps) = if g.h265 {
            ("h265parse", "video/x-h265,stream-format=byte-stream,alignment=au")
        } else {
            ("h264parse", "video/x-h264,stream-format=byte-stream,alignment=au")
        };
        let head = format!("{parse} ! decodebin ! {convert} name=tail");
        let head = head.as_str();
        g.video = Some(self.start(g, head, caps, &self.video_sink, true, "video")?);
        Ok(())
    }

    fn start_audio(&self, g: &mut Inner, codec: AudioCodec) -> Result<(), String> {
        if let Some((_, p)) = g.audio.take() {
            p.stop();
        }
        let (dec, caps) = match codec {
            AudioCodec::Alac => ("avdec_alac", ALAC_CAPS),
            AudioCodec::AacEld => ("avdec_aac", AAC_ELD_CAPS),
            AudioCodec::Aac => ("avdec_aac", AAC_LC_CAPS),
        };
        let head = format!("queue ! {dec} ! audioconvert ! audioresample quality=10 name=tail");
        g.audio = Some((codec, self.start(g, &head, caps, &self.audio_sink, false, "audio")?));
        Ok(())
    }

    fn push(src: &gst_app::AppSrc, data: &[u8], pts: gst::ClockTime) -> bool {
        let mut buf = gst::Buffer::from_slice(data.to_vec());
        buf.get_mut().unwrap().set_pts(pts);
        src.push_buffer(buf).is_ok()
    }
}

impl MediaSink for GstSink {
    fn video_codec(&self, _width: u16, _height: u16, h265: bool) {
        let mut g = self.lock();
        g.h265 = h265;
        if let Err(e) = self.start_video(&mut g) {
            self.note(&format!("video pipeline failed: {e}"));
        }
    }

    fn video(&self, annexb: &[u8], pts_ns: u64) {
        let mut g = self.lock();
        if g.video.is_none() {
            if let Err(e) = self.start_video(&mut g) {
                self.note(&format!("video pipeline failed: {e}"));
                return;
            }
        }
        let pts = self.timestamp(&mut g, pts_ns);
        let ok = g.video.as_ref().map(|p| Self::push(&p.src, annexb, pts)).unwrap_or(true);
        if !ok {
            if let Some(p) = g.video.take() {
                p.stop();
            }
        }
    }

    fn audio(&self, codec: AudioCodec, data: &[u8], pts_ns: u64) {
        let mut g = self.lock();
        if g.audio.as_ref().map(|(c, _)| *c) != Some(codec) {
            if let Err(e) = self.start_audio(&mut g, codec) {
                self.note(&format!("audio pipeline failed: {e}"));
                return;
            }
        }
        let pts = self.timestamp(&mut g, pts_ns);
        let ok = g.audio.as_ref().map(|(_, p)| Self::push(&p.src, data, pts)).unwrap_or(true);
        if !ok {
            if let Some((_, p)) = g.audio.take() {
                p.stop();
            }
        }
    }

    fn flush(&self) {
        let mut g = self.lock();
        if let Some(p) = &g.video {
            p.flush();
        }
        if let Some((_, p)) = &g.audio {
            p.flush();
        }
        g.offset = None;
    }

    fn connection(&self, _open: bool) {
        self.lock().reset();
    }

    fn log(&self, level: i32, msg: &str) {
        if level <= 1 {
            eprintln!("airplay-render: {msg}");
        }
    }
}

impl Drop for GstSink {
    fn drop(&mut self) {
        self.lock().reset();
    }
}
