use plugin_capture_direct::mock_backend::MockDirectReceiverBackend;

#[tokio::test]
async fn direct_receiver_backend_reports_unavailable_without_helper() {
    let backend = MockDirectReceiverBackend::unavailable("helper missing");
    let result = backend.start_session().await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("helper missing"));
}
