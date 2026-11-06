#![allow(non_camel_case_types)]

use crate::sink::{AudioCodec, MediaSink};
use std::ffi::{c_char, c_double, c_float, c_int, c_uchar, c_ushort, c_void, CStr, CString};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

#[repr(C)]
struct dnssd_t {
    _p: [u8; 0],
}
#[repr(C)]
struct raop_t {
    _p: [u8; 0],
}
#[repr(C)]
struct raop_ntp_t {
    _p: [u8; 0],
}

#[repr(C)]
struct video_decode_struct {
    is_h265: bool,
    nal_count: c_int,
    data: *mut c_uchar,
    data_len: c_int,
    ntp_time_local: u64,
    ntp_time_remote: u64,
}

#[repr(C)]
struct audio_decode_struct {
    data: *mut c_uchar,
    ct: c_uchar,
    data_len: c_int,
    sync_status: c_int,
    ntp_time_local: u64,
    ntp_time_remote: u64,
    rtp_time: u32,
    seqnum: c_ushort,
}

type Cls = *mut c_void;

#[repr(C)]
struct raop_callbacks_t {
    cls: Cls,
    audio_process: Option<unsafe extern "C" fn(Cls, *mut raop_ntp_t, *mut audio_decode_struct)>,
    video_process: Option<unsafe extern "C" fn(Cls, *mut raop_ntp_t, *mut video_decode_struct)>,
    video_pause: Option<unsafe extern "C" fn(Cls)>,
    video_resume: Option<unsafe extern "C" fn(Cls)>,
    conn_feedback: Option<unsafe extern "C" fn(Cls)>,
    conn_reset: Option<unsafe extern "C" fn(Cls, c_int)>,
    video_reset: Option<unsafe extern "C" fn(Cls, c_int)>,
    video_set_codec_and_size: Option<
        unsafe extern "C" fn(Cls, c_int, c_ushort, c_ushort, c_ushort, c_ushort, c_ushort, c_ushort) -> c_int,
    >,
    conn_init: Option<unsafe extern "C" fn(Cls)>,
    conn_destroy: Option<unsafe extern "C" fn(Cls)>,
    audio_flush: Option<unsafe extern "C" fn(Cls)>,
    video_flush: Option<unsafe extern "C" fn(Cls)>,
    audio_set_client_volume: Option<unsafe extern "C" fn(Cls) -> c_double>,
    audio_set_volume: Option<unsafe extern "C" fn(Cls, c_float)>,
    audio_set_metadata: Option<unsafe extern "C" fn(Cls, *const c_void, c_int)>,
    audio_set_coverart: Option<unsafe extern "C" fn(Cls, *const c_void, c_int)>,
    audio_stop_coverart_rendering: Option<unsafe extern "C" fn(Cls)>,
    audio_remote_control_id: Option<unsafe extern "C" fn(Cls, *const c_char, *const c_char)>,
    audio_set_progress: Option<unsafe extern "C" fn(Cls, *mut u32, *mut u32, *mut u32)>,
    audio_get_format:
        Option<unsafe extern "C" fn(Cls, *mut c_uchar, *mut c_ushort, *mut bool, *mut bool, *mut u64)>,
    mirror_video_running: Option<unsafe extern "C" fn(Cls, bool)>,
    report_client_request: Option<unsafe extern "C" fn(Cls, *mut c_char, *mut c_char, *mut c_char, *mut bool)>,
    display_pin: Option<unsafe extern "C" fn(Cls, *mut c_char)>,
    register_client: Option<unsafe extern "C" fn(Cls, *const c_char, *const c_char, *const c_char)>,
    check_register: Option<unsafe extern "C" fn(Cls, *const c_char) -> bool>,
    passwd: Option<unsafe extern "C" fn(Cls, *mut c_int) -> *const c_char>,
    export_dacp: Option<unsafe extern "C" fn(Cls, *const c_char, *const c_char)>,
    on_video_play: Option<unsafe extern "C" fn(Cls, *const c_char, c_float)>,
    on_video_scrub: Option<unsafe extern "C" fn(Cls, c_float)>,
    on_video_rate: Option<unsafe extern "C" fn(Cls, c_float)>,
    on_video_stop: Option<unsafe extern "C" fn(Cls)>,
    on_video_acquire_playback_info: Option<unsafe extern "C" fn(Cls, *mut c_void)>,
    on_video_playlist_remove: Option<unsafe extern "C" fn(Cls) -> c_float>,
    get_custom_profile: Option<unsafe extern "C" fn(Cls, *mut *const c_char)>,
}

extern "C" {
    fn free(p: *mut c_void);
    fn dnssd_init(
        name: *const c_char,
        name_len: c_int,
        hw_addr: *const c_char,
        hw_addr_len: c_int,
        pin_pw: c_uchar,
        error: *mut c_int,
    ) -> *mut dnssd_t;
    fn dnssd_set_airplay_features(d: *mut dnssd_t, bit: c_int, val: c_int);
    fn dnssd_register_raop(d: *mut dnssd_t, port: c_ushort) -> c_int;
    fn dnssd_register_airplay(d: *mut dnssd_t, port: c_ushort) -> c_int;
    fn dnssd_unregister_raop(d: *mut dnssd_t);
    fn dnssd_unregister_airplay(d: *mut dnssd_t);
    fn dnssd_destroy(d: *mut dnssd_t);
    fn raop_init(cb: *mut raop_callbacks_t) -> *mut raop_t;
    fn raop_init2(r: *mut raop_t, nohold: c_int, device_id: *const c_char, keyfile: *const c_char) -> c_int;
    fn raop_set_log_level(r: *mut raop_t, level: c_int);
    fn raop_set_log_callback(r: *mut raop_t, cb: unsafe extern "C" fn(Cls, c_int, *const c_char), cls: Cls);
    fn raop_set_tcp_ports(r: *mut raop_t, p: *mut c_ushort);
    fn raop_set_udp_ports(r: *mut raop_t, p: *mut c_ushort);
    fn raop_get_port(r: *mut raop_t) -> c_ushort;
    fn raop_set_port(r: *mut raop_t, port: c_ushort);
    fn raop_start_httpd(r: *mut raop_t, port: *mut c_ushort) -> c_int;
    fn raop_set_dnssd(r: *mut raop_t, d: *mut dnssd_t);
    fn raop_destroy(r: *mut raop_t);
    fn get_local_time() -> u64;
    fn ntp_global_init();
}

const LOGGER_INFO: c_int = 6;
const LOGGER_DEBUG: c_int = 7;
const RESET_TYPE_ON_VIDEO_PLAY: c_int = 4;
const VIDEO_CODEC_H265: c_int = 2;

struct Ctx {
    sink: Arc<dyn MediaSink>,
    clock_offset: Mutex<Option<u64>>,
    open: AtomicI32,
}

impl Ctx {
    fn pts(&self, local: u64, remote: u64) -> u64 {
        let mut off = self.clock_offset.lock().unwrap();
        let o = *off.get_or_insert_with(|| {
            let l = if local != 0 { local } else { unsafe { get_local_time() } };
            l.wrapping_sub(remote)
        });
        remote.wrapping_add(o)
    }

    fn reset_clock(&self) {
        *self.clock_offset.lock().unwrap() = None;
    }
}

unsafe fn ctx<'a>(cls: Cls) -> &'a Ctx {
    &*(cls as *const Ctx)
}

unsafe extern "C" fn audio_process(cls: Cls, _: *mut raop_ntp_t, d: *mut audio_decode_struct) {
    let c = ctx(cls);
    let d = &*d;
    let codec = match d.ct {
        2 => AudioCodec::Alac,
        8 => AudioCodec::AacEld,
        4 => AudioCodec::Aac,
        _ => return,
    };
    if d.data.is_null() || d.data_len <= 0 {
        return;
    }
    let pts = c.pts(d.ntp_time_local, d.ntp_time_remote);
    c.sink.audio(codec, std::slice::from_raw_parts(d.data, d.data_len as usize), pts);
}

unsafe extern "C" fn video_process(cls: Cls, _: *mut raop_ntp_t, d: *mut video_decode_struct) {
    let c = ctx(cls);
    let d = &*d;
    if d.data.is_null() || d.data_len <= 0 {
        return;
    }
    let pts = c.pts(d.ntp_time_local, d.ntp_time_remote);
    c.sink.video(std::slice::from_raw_parts(d.data, d.data_len as usize), pts);
}

unsafe extern "C" fn video_set_codec_and_size(
    cls: Cls,
    codec: c_int,
    _ws: c_ushort,
    _hs: c_ushort,
    _x: c_ushort,
    _y: c_ushort,
    w: c_ushort,
    h: c_ushort,
) -> c_int {
    ctx(cls).sink.video_codec(w, h, codec == VIDEO_CODEC_H265);
    0
}

unsafe extern "C" fn noop(_: Cls) {}

unsafe extern "C" fn flush(cls: Cls) {
    ctx(cls).sink.flush();
}

unsafe extern "C" fn conn_reset(cls: Cls, _reason: c_int) {
    let c = ctx(cls);
    c.reset_clock();
    c.sink.flush();
}

unsafe extern "C" fn video_reset(cls: Cls, kind: c_int) {
    if kind == RESET_TYPE_ON_VIDEO_PLAY {
        return;
    }
    let c = ctx(cls);
    c.reset_clock();
    c.sink.flush();
}

unsafe extern "C" fn conn_init(cls: Cls) {
    let c = ctx(cls);
    if c.open.fetch_add(1, Ordering::SeqCst) == 0 {
        c.sink.connection(true);
    }
}

unsafe extern "C" fn conn_destroy(cls: Cls) {
    let c = ctx(cls);
    if c.open.fetch_sub(1, Ordering::SeqCst) == 1 {
        c.reset_clock();
        c.sink.connection(false);
    }
}

unsafe extern "C" fn client_volume(_: Cls) -> c_double {
    0.0
}

unsafe extern "C" fn set_volume(_: Cls, _: c_float) {}

unsafe extern "C" fn set_blob(_: Cls, _: *const c_void, _: c_int) {}

unsafe extern "C" fn display_pin(_: Cls, _: *mut c_char) {}

unsafe extern "C" fn check_register(_: Cls, _: *const c_char) -> bool {
    true
}

unsafe extern "C" fn audio_get_format(
    cls: Cls,
    ct: *mut c_uchar,
    _spf: *mut c_ushort,
    _screen: *mut bool,
    _media: *mut bool,
    _fmt: *mut u64,
) {
    let c = ctx(cls);
    c.sink.log(LOGGER_INFO, &format!("audio stream ct={}", *ct));
}

unsafe extern "C" fn log_cb(cls: Cls, level: c_int, msg: *const c_char) {
    let c = ctx(cls);
    c.sink.log(level, &CStr::from_ptr(msg).to_string_lossy());
}

pub struct Engine {
    ctx: *mut Ctx,
    dnssd: *mut dnssd_t,
    raop: *mut raop_t,
    port: u16,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

fn keyfile_path() -> PathBuf {
    match std::env::var_os("LOVEDSCREEN_KEYFILE") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => std::env::temp_dir().join("lovedscreen-airplay.pem"),
    }
}

fn prepare_keyfile(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    match o.open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(format!("keyfile {}: {e}", path.display())),
    }
}

fn hw_addr(name: &str) -> [u8; 6] {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let mut a = [0u8; 6];
    a.copy_from_slice(&h.to_be_bytes()[2..]);
    a[0] = (a[0] & !1) | 2;
    a
}

impl Engine {
    pub fn start(name: &str, sink: Arc<dyn MediaSink>) -> Result<Engine, String> {
        static NTP_INIT: std::sync::Once = std::sync::Once::new();
        NTP_INIT.call_once(|| unsafe { ntp_global_init() });
        let cname = CString::new(name).map_err(|_| "name contains NUL".to_string())?;
        let keyfile = keyfile_path();
        prepare_keyfile(&keyfile)?;
        let ckey = CString::new(keyfile.to_string_lossy().as_bytes()).map_err(|_| "bad keyfile path".to_string())?;
        let hw = hw_addr(name);
        let mac = hw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":");
        let cmac = CString::new(mac).unwrap();

        let mut e = Engine {
            ctx: Box::into_raw(Box::new(Ctx {
                sink,
                clock_offset: Mutex::new(None),
                open: AtomicI32::new(0),
            })),
            dnssd: ptr::null_mut(),
            raop: ptr::null_mut(),
            port: 0,
        };

        unsafe {
            let mut err: c_int = 0;
            e.dnssd = dnssd_init(
                cname.as_ptr(),
                name.len() as c_int,
                hw.as_ptr() as *const c_char,
                6,
                0,
                &mut err,
            );
            if e.dnssd.is_null() || err != 0 {
                if !e.dnssd.is_null() {
                    dnssd_destroy(e.dnssd);
                    e.dnssd = ptr::null_mut();
                }
                return Err(format!("dnssd_init failed: {err}"));
            }
            dnssd_set_airplay_features(e.dnssd, 27, 0);
            dnssd_set_airplay_features(e.dnssd, 42, 0);

            let mut cbs: raop_callbacks_t = std::mem::zeroed();
            cbs.cls = e.ctx as Cls;
            cbs.audio_process = Some(audio_process);
            cbs.video_process = Some(video_process);
            cbs.video_pause = Some(noop);
            cbs.video_resume = Some(noop);
            cbs.conn_feedback = Some(noop);
            cbs.conn_reset = Some(conn_reset);
            cbs.video_reset = Some(video_reset);
            cbs.video_set_codec_and_size = Some(video_set_codec_and_size);
            cbs.conn_init = Some(conn_init);
            cbs.conn_destroy = Some(conn_destroy);
            cbs.audio_flush = Some(flush);
            cbs.video_flush = Some(flush);
            cbs.audio_set_client_volume = Some(client_volume);
            cbs.audio_set_volume = Some(set_volume);
            cbs.audio_set_metadata = Some(set_blob);
            cbs.audio_set_coverart = Some(set_blob);
            cbs.audio_stop_coverart_rendering = Some(noop);
            cbs.audio_get_format = Some(audio_get_format);
            cbs.display_pin = Some(display_pin);
            cbs.check_register = Some(check_register);

            e.raop = raop_init(&mut cbs);
            if e.raop.is_null() {
                return Err("raop_init failed".into());
            }
            raop_set_log_callback(e.raop, log_cb, e.ctx as Cls);
            raop_set_log_level(e.raop, LOGGER_DEBUG);
            if raop_init2(e.raop, 0, cmac.as_ptr(), ckey.as_ptr()) != 0 {
                free(e.raop as *mut c_void);
                e.raop = ptr::null_mut();
                return Err("raop_init2 failed".into());
            }
            let mut tcp: [c_ushort; 3] = [0; 3];
            let mut udp: [c_ushort; 3] = [0; 3];
            raop_set_tcp_ports(e.raop, tcp.as_mut_ptr());
            raop_set_udp_ports(e.raop, udp.as_mut_ptr());
            let mut port = raop_get_port(e.raop);
            if raop_start_httpd(e.raop, &mut port) < 0 || port == 0 {
                return Err("raop_start_httpd failed".into());
            }
            raop_set_port(e.raop, port);
            e.port = port;
            raop_set_dnssd(e.raop, e.dnssd);
            let r = dnssd_register_raop(e.dnssd, port);
            if r != 0 {
                return Err(format!("dnssd_register_raop failed: {r}"));
            }
            let r = dnssd_register_airplay(e.dnssd, port);
            if r != 0 {
                return Err(format!("dnssd_register_airplay failed: {r}"));
            }
        }
        Ok(e)
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            if !self.raop.is_null() {
                raop_destroy(self.raop);
            }
            if !self.dnssd.is_null() {
                dnssd_unregister_raop(self.dnssd);
                dnssd_unregister_airplay(self.dnssd);
                dnssd_destroy(self.dnssd);
            }
            drop(Box::from_raw(self.ctx));
        }
    }
}
