# Host Desktop User Data Logs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist host-desktop diagnostics to a per-launch log file in the user data `logs/` folder.

**Architecture:** Reuse host preference path derivation to locate the app data root, add a small host log writer module for per-launch files, and mirror existing diagnostics events into both the UI model and the launch log file. Logging failures stay non-fatal.

**Tech Stack:** Rust, std filesystem APIs, existing host-desktop test suite

---

### Task 1: Add Failing Tests For Log Paths And Launch Files

**Files:**
- Modify: `apps/host-desktop/src/preferences.rs`
- Modify: `apps/host-desktop/tests/app_state.rs`

- [ ] **Step 1: Write the failing path test**

```rust
#[test]
fn log_directory_is_sibling_to_preferences_file() {
    let prefs = PathBuf::from("/tmp/app/ios-control/host-preferences.json");
    assert_eq!(
        HostPreferencesStore::log_dir_for_preferences_path(&prefs),
        PathBuf::from("/tmp/app/ios-control/logs")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop log_directory_is_sibling_to_preferences_file -- --exact`
Expected: FAIL with missing `log_dir_for_preferences_path`

- [ ] **Step 3: Write the failing host app log file test**

```rust
#[test]
fn host_app_writes_launch_logs_into_user_data_logs_folder() {
    let mut fixture = host_app_with_runtime_and_preferences("{}");
    fixture.app.request_start_session();
    let prefs_path = fixture.preferences_path.as_ref().unwrap();
    let logs_dir = HostPreferencesStore::log_dir_for_preferences_path(prefs_path);
    let entries = std::fs::read_dir(&logs_dir).unwrap().collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p host-desktop host_app_writes_launch_logs_into_user_data_logs_folder -- --exact`
Expected: FAIL because no log file is created

### Task 2: Implement Path Helpers And Host Log Writer

**Files:**
- Modify: `apps/host-desktop/src/preferences.rs`
- Create: `apps/host-desktop/src/logging.rs`
- Modify: `apps/host-desktop/src/lib.rs`

- [ ] **Step 1: Add log directory helpers**

```rust
pub fn log_dir_for_preferences_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("logs")
}
```

- [ ] **Step 2: Add a per-launch host log writer**

```rust
pub struct HostLogWriter {
    path: PathBuf,
}
```

- [ ] **Step 3: Implement append behavior**

```rust
pub fn append_line(&self, line: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&self.path)?;
    writeln!(file, "{line}")?;
    Ok(())
}
```

### Task 3: Wire Host Diagnostics Into Persistent Launch Logs

**Files:**
- Modify: `apps/host-desktop/src/app.rs`
- Modify: `apps/host-desktop/src/view_models/diagnostics.rs`

- [ ] **Step 1: Initialize the writer for preferences-backed app startup**

```rust
app.install_log_writer_from_preferences_path(store.path());
```

- [ ] **Step 2: Flush existing in-memory lines after writer installation**

```rust
for line in app.diagnostics.log_lines.clone() {
    app.append_host_log_line(&line);
}
```

- [ ] **Step 3: Mirror future diagnostics events to disk**

```rust
let line = self.diagnostics.record_inventory_snapshot(&snapshot);
self.append_host_log_line(&line);
```

- [ ] **Step 4: Surface non-fatal write failures**

```rust
eprintln!("warning: failed to append host log line: {error}");
```

### Task 4: Verify

**Files:**
- Test: `apps/host-desktop/tests/app_state.rs`
- Test: `apps/host-desktop/src/preferences.rs`

- [ ] **Step 1: Run targeted tests**

Run: `cargo test -p host-desktop log_directory_is_sibling_to_preferences_file -- --exact`
Expected: PASS

- [ ] **Step 2: Run targeted host logging test**

Run: `cargo test -p host-desktop host_app_writes_launch_logs_into_user_data_logs_folder -- --exact`
Expected: PASS

- [ ] **Step 3: Run package suite**

Run: `cargo test -p host-desktop`
Expected: PASS
