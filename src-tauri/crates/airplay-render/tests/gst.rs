use airplay_core::{AudioCodec, MediaSink};
use airplay_render::GstSink;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn need(names: &[&str]) {
    gst::init().unwrap();
    for n in names {
        assert!(gst::ElementFactory::find(n).is_some(), "missing gstreamer element {n}");
    }
}

fn encode(desc: &str) -> (Vec<(Vec<u8>, u64)>, Option<Vec<u8>>) {
    gst::init().unwrap();
    let pipeline = gst::parse::launch(&format!("{desc} ! appsink name=out sync=false")).unwrap();
    let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();
    let out = pipeline.by_name("out").unwrap().downcast::<gst_app::AppSink>().unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();
    let mut frames = Vec::new();
    let mut codec_data = None;
    let mut n = 0u64;
    while let Ok(sample) = out.pull_sample() {
        if codec_data.is_none() {
            codec_data = sample
                .caps()
                .and_then(|c| c.structure(0))
                .and_then(|s| s.get::<gst::Buffer>("codec_data").ok())
                .map(|b| b.map_readable().unwrap().to_vec());
        }
        let buf = sample.buffer().unwrap();
        frames.push((buf.map_readable().unwrap().to_vec(), n));
        n += 1;
    }
    pipeline.set_state(gst::State::Null).unwrap();
    (frames, codec_data)
}

fn counter(pipeline: &gst::Pipeline, name: &str) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let pad = pipeline.by_name(name).unwrap().static_pad("sink").unwrap();
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
        c.fetch_add(1, Ordering::SeqCst);
        gst::PadProbeReturn::Ok
    });
    count
}

fn wait_for(count: &AtomicUsize, at_least: usize) -> usize {
    let end = Instant::now() + Duration::from_secs(10);
    while count.load(Ordering::SeqCst) < at_least && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    count.load(Ordering::SeqCst)
}

fn h264() -> Vec<(Vec<u8>, u64)> {
    need(&["x264enc", "h264parse", "decodebin", "avdec_h264"]);
    encode("videotestsrc num-buffers=30 ! video/x-raw,width=320,height=240,framerate=30/1 ! x264enc tune=zerolatency key-int-max=10 byte-stream=true ! video/x-h264,stream-format=byte-stream,alignment=au").0
}

fn sink() -> GstSink {
    GstSink::with_sinks("fakesink name=vs", "fakesink name=as").unwrap()
}

#[test]
fn video_frames_reach_sink() {
    let frames = h264();
    assert!(frames.len() >= 30);
    let s = sink();
    s.video_codec(320, 240, false);
    let count = counter(&s.pipelines().0.unwrap(), "vs");
    for (data, i) in &frames {
        s.video(data, 1_000_000_000_000 + i * 33_000_000);
    }
    assert!(wait_for(&count, 25) >= 25);
}

#[test]
fn video_flush_then_resume_and_reconnect() {
    let frames = h264();
    let s = sink();
    s.video_codec(320, 240, false);
    let count = counter(&s.pipelines().0.unwrap(), "vs");
    for (data, i) in frames.iter().take(15) {
        s.video(data, i * 33_000_000);
    }
    assert!(wait_for(&count, 10) >= 10);
    s.flush();
    let before = count.load(Ordering::SeqCst);
    for (data, i) in frames.iter().skip(15) {
        s.video(data, 2_000_000_000 + i * 33_000_000);
    }
    assert!(wait_for(&count, before + 10) >= before + 10);

    s.connection(false);
    assert!(s.pipelines().0.is_none());
    s.connection(true);
    s.video_codec(320, 240, false);
    let count = counter(&s.pipelines().0.unwrap(), "vs");
    for (data, i) in &frames {
        s.video(data, i * 33_000_000);
    }
    assert!(wait_for(&count, 25) >= 25);
}

fn alac_frames(n: u64) -> Vec<(Vec<u8>, u64)> {
    (0..n)
        .map(|i| {
            let mut bits: Vec<bool> = Vec::new();
            let mut put = |v: u32, w: u32| (0..w).rev().for_each(|k| bits.push(v >> k & 1 == 1));
            put(1, 3);
            put(0, 4);
            put(0, 12);
            put(0, 1);
            put(0, 2);
            put(1, 1);
            for k in 0..352u32 {
                let v = ((k * 300 + i as u32 * 7) & 0x3fff) as u32;
                put(v, 16);
                put(v, 16);
            }
            put(7, 3);
            while bits.len() % 8 != 0 {
                bits.push(false);
            }
            let bytes = bits.chunks(8).map(|c| c.iter().fold(0u8, |a, b| a << 1 | *b as u8)).collect();
            (bytes, i)
        })
        .collect()
}

fn audio_roundtrip(frames: Vec<(Vec<u8>, u64)>, codec: AudioCodec) {
    need(&["avdec_aac", "avdec_alac"]);
    assert!(frames.len() >= 20);
    let s = sink();
    s.audio(codec, &frames[0].0, 0);
    let count = counter(&s.pipelines().1.unwrap(), "as");
    for (data, i) in frames.iter().skip(1) {
        s.audio(codec, data, i * 23_000_000);
    }
    let want = frames.len() / 2;
    assert!(wait_for(&count, want) >= want);
    s.flush();
    let before = count.load(Ordering::SeqCst);
    for (data, i) in frames.iter().take(10) {
        s.audio(codec, data, 5_000_000_000 + i * 23_000_000);
    }
    assert!(wait_for(&count, before + 5) >= before + 5);
}

#[test]
fn aac_lc_frames_reach_sink() {
    need(&["avenc_aac"]);
    let (frames, codec_data) = encode(
        "audiotestsrc num-buffers=200 samplesperbuffer=1024 ! audio/x-raw,rate=44100,channels=2 ! audioconvert ! avenc_aac ! audio/mpeg,mpegversion=4,stream-format=raw",
    );
    let hex: String = codec_data.unwrap().iter().map(|b| format!("{b:02x}")).collect();
    assert!(hex.starts_with("1210"), "{hex}");
    audio_roundtrip(frames, AudioCodec::Aac);
}

#[test]
fn alac_frames_reach_sink() {
    audio_roundtrip(alac_frames(100), AudioCodec::Alac);
}

#[test]
fn drop_does_not_hang() {
    let frames = h264();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let s = sink();
        s.video_codec(320, 240, false);
        for (data, i) in frames.iter().take(5) {
            s.video(data, i * 33_000_000);
        }
        s.audio(AudioCodec::Aac, &[0u8; 8], 0);
        drop(s);
        tx.send(()).unwrap();
    });
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
}
