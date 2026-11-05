use airplay_core::{AudioCodec, MediaSink, Server};
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Rec {
    events: Mutex<Vec<String>>,
}

impl MediaSink for Rec {
    fn video_codec(&self, w: u16, h: u16, h265: bool) {
        self.events.lock().unwrap().push(format!("codec {w}x{h} {h265}"));
    }
    fn video(&self, d: &[u8], pts: u64) {
        self.events.lock().unwrap().push(format!("video {} {pts}", d.len()));
    }
    fn audio(&self, c: AudioCodec, d: &[u8], pts: u64) {
        self.events.lock().unwrap().push(format!("audio {c:?} {} {pts}", d.len()));
    }
    fn flush(&self) {
        self.events.lock().unwrap().push("flush".into());
    }
    fn connection(&self, open: bool) {
        self.events.lock().unwrap().push(format!("conn {open}"));
    }
    fn log(&self, level: i32, msg: &str) {
        self.events.lock().unwrap().push(format!("log {level} {msg}"));
    }
}

fn rtsp(port: u16, req: &str) -> Vec<u8> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(req.as_bytes()).unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if let Some(h) = out.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&out[..h]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if out.len() >= h + 4 + len {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
    out
}

fn query(name: &str) -> Vec<u8> {
    let mut p = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for l in name.split('.') {
        p.push(l.len() as u8);
        p.extend_from_slice(l.as_bytes());
    }
    p.extend_from_slice(&[0, 0, 12, 0x80, 1]);
    p
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn mdns_ptr(name: &str, instance: &str) {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    sock.send_to(&query(name), "127.0.0.1:5353").unwrap();
    let mut buf = [0u8; 4096];
    let (n, _) = sock.recv_from(&mut buf).expect("no mDNS reply");
    let r = &buf[..n];
    assert_eq!(r[2] & 0x80, 0x80, "not a response");
    assert!(u16::from_be_bytes([r[6], r[7]]) >= 1, "no answers");
    assert!(contains(r, name.split('.').next().unwrap().as_bytes()));
    assert!(contains(r, instance.as_bytes()), "instance missing in {name} reply");
}

#[test]
fn serves_rtsp_and_mdns_and_restarts() {
    for round in 0..2 {
        let rec = Arc::new(Rec::default());
        let server = Server::start("LovedscreenTest", rec.clone()).expect("start");
        let port = server.port();
        assert!(port > 0);

        let info = rtsp(port, "GET /info RTSP/1.0\r\nCSeq: 1\r\nUser-Agent: test\r\n\r\n");
        let text = String::from_utf8_lossy(&info);
        assert!(text.starts_with("RTSP/1.0 200 OK"), "info: {text}");
        let body = info.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(info[body..].starts_with(b"bplist00"), "info body is not a plist");
        println!("round {round}: GET /info -> {} bytes, bplist body {}", info.len(), info.len() - body);

        let opt = rtsp(port, "OPTIONS * RTSP/1.0\r\nCSeq: 2\r\n\r\n");
        let text = String::from_utf8_lossy(&opt);
        assert!(text.starts_with("RTSP/1.0 200 OK") && text.contains("Public:"), "options: {text}");
        println!("round {round}: OPTIONS -> {}", text.lines().take(4).collect::<Vec<_>>().join(" | "));

        mdns_ptr("_airplay._tcp.local", "LovedscreenTest");
        mdns_ptr("_raop._tcp.local", "LovedscreenTest");
        println!("round {round}: mDNS PTR _airplay._tcp.local and _raop._tcp.local answered");

        std::thread::sleep(Duration::from_millis(200));
        let ev = rec.events.lock().unwrap().clone();
        assert!(ev.iter().any(|e| e == "conn true") && ev.iter().any(|e| e == "conn false"));
        assert!(ev.iter().any(|e| e.starts_with("log ")));
        drop(server);
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "port still open after drop");
    }
}
