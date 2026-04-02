use plugin_capture_window::mock_backend::MockWindowBackend;

#[tokio::test]
async fn window_capture_lists_mock_source_then_streams_one_frame() {
    let mut backend = MockWindowBackend::default();
    let sources = backend.list_sources().await.unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, "window:mock");

    let frame = backend.next_frame("window:mock").await.unwrap();
    assert_eq!(frame.frame_index, 1);
    assert_eq!(frame.width, 1280);
}
