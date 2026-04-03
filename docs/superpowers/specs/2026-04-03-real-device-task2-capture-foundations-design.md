## Scope

Implement Task 2 foundations for capture-window and capture-direct plugins. Replace trivial `cfg!()` probes with conservative runtime checks, introduce a minimal live stream lifecycle over the new protocol (`OpenCaptureStream`, `ReadCaptureFrame`, `CloseCaptureStream`), and keep existing mock and `GetCaptureFrame` flows working for tests and compatibility. No real OS capture engines or helper transport are implemented in this task.

## Goals

- Runtime probes reflect actual environment readiness and default to `false` in generic CI.
- Main loops support live stream open/read/close while still serving `GetCaptureFrame`.
- Capture path writes mock frame bytes into `FrameSlot` to exercise exact-write behavior.
- Direct capture integrates helper discovery meaningfully, returning errors when helper is missing.
- Tests cover new runtime probe behavior and keep existing mock tests passing.

## Non-Goals

- Real window capture (Wayland/X11/Windows APIs), or real direct receiver transport.
- Full helper process lifecycle beyond existence/availability checks.
- Changing protocol contracts or adding new IPC messages.

## Design

### Runtime probes

- **Linux window probe**: return `true` only when `WAYLAND_DISPLAY` or `DISPLAY` is set at runtime. Otherwise `false`. This keeps CI conservative and avoids `cfg!()`-only answers.
- **Windows window probe**: return `true` only when the OS is Windows **and** the runtime indicates an interactive session. Use a conservative signal (`SESSIONNAME` env var present) to avoid false positives in CI containers. On non-Windows targets, always `false`.

### Stream lifecycle

Maintain a simple in-process stream state in each `main.rs`:

- `OpenCaptureStream`: validate source/helper availability, create a `FrameSlot`, reply `CaptureStreamOpened`.
- `ReadCaptureFrame`: if no open stream, return an error; otherwise write mock pixel bytes to the slot (exact length) and reply with `CaptureFrame` using `FrameHealth::Healthy` and incremented `frame_index`.
- `CloseCaptureStream`: clear the open stream state, reply `Ack`.

### Backward compatibility

- `GetCaptureFrame` remains supported and returns a mock frame (without requiring an open stream) to keep existing mock/runtime tests and clients working.
- `ListCaptureSources` for window capture remains unchanged.

### Direct helper integration

The direct path uses `helper_launcher::find_helper()`:

- If helper is not set, `OpenCaptureStream` and `StartDirectCapture` should return a clear error.
- If helper is set, stream can be opened and mock frames returned.

## Error handling

- Missing helper or unsupported runtime yields `PluginToHost::Error` with a specific message.
- `ReadCaptureFrame` when no stream is open yields an error.
- Unknown source IDs yield errors.

## Testing

- Add Linux probe test asserting `probe_linux_capture()` returns `false` by default in CI.
- Add Windows probe test asserting `probe_windows_capture()` returns `false` by default in CI.
- Keep existing mock backend tests unchanged and passing.
- Run:
  - `cargo test -p plugin-capture-window`
  - `cargo test -p plugin-capture-direct`

