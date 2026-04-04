use std::fs;
use std::path::Path;

#[test]
fn readme_matches_current_protocol_and_mock_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let readme = fs::read_to_string(root.join("README.md")).unwrap();

    assert!(readme.contains("protocol version `3`"));
    assert!(readme.contains("window-helper-1"));
}
