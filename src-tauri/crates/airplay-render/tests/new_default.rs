use airplay_render::GstSink;

#[test]
fn new_initializes_gstreamer_itself() {
    assert!(GstSink::new().is_ok());
}
