use std::path::PathBuf;

use ios_control_plugin_runtime::PluginRuntime;

#[tokio::test]
async fn handshake_returns_mock_plugin_descriptor() {
    let runtime = PluginRuntime::new();
    let plugin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/debug/plugin-mock-device{}",
        std::env::consts::EXE_SUFFIX
    ));

    let descriptor = runtime.handshake(&plugin_path).await.unwrap();

    assert_eq!(descriptor.plugin_id, "mock.device");
    assert_eq!(descriptor.protocol_version, 1);
}
