# Bug Review — Code-Verified Findings

Status: in progress (started 2026-07-01)

This document collects **code-verified** bugs found by deep horizontal + vertical
review of the `ios-control` Rust workspace. Each entry has been re-checked against
the actual source before being recorded here. Entries are numbered `BUG-###`.

Legend:
- **Severity** — High (correctness / crash / deadlock / data loss), Medium (logic flaw / robustness), Low (quality / latent risk).
- Failure scenario = the concrete input/state that triggers the wrong behavior.

---

## High severity

### BUG-001 — start_or_replace_session starts the new capture session before shutting down the previous one (collides on fixed AirPlay ports + mDNS identity)
File: crates/session-orchestrator/src/lib.rs:318-333

start_or_replace_session calls session_actor::start_session_actor(...) at line 323 — which runs the full start_session_with_plugins pipeline (spawns capture plugin, handshakes, opens the capture stream, and for capture.direct calls DirectRuntimeSession::start) — and only after that succeeds does it shut the previous session down (previous.shutdown().await? at line 327).

For the capture.direct backend, DirectRuntimeSession::start (plugins/capture-direct/src/uxplay_launcher.rs:40-81) advertises AirPlay/RAOP mDNS services on a hardcoded RTSP port (AIRPLAY_RTSP_PORT = 52082, uxplay_launcher.rs:14,80) with a fixed identity-derived name, and binds the -p 52081 base port (uxplay_launcher.rs:13,231). The previous session still holds that port + mDNS registration when the new one starts.

Failure scenario: the operator clicks Start on a device that is already streaming (reconnect/restart). The new capture.direct session launches UxPlay and tries to register the same _airplay._tcp / _raop._tcp service name on port 52082 while the old session still owns it, so airplay_mdns.rs:213 fails with "failed to register AirPlay mDNS service" and/or the RTSP bind fails. The new session errors out and the old one is then torn down too — the user loses the working stream. Fix: shut down the previous session before starting the new one.

### BUG-002 — ble-helper `execute` discards the requested plan kind and always runs a hardcoded mouse wiggle
Files: helpers/ble-helper/src/windows_hid.rs:368-377 (and main.rs:106-124, plugins/control-ble/src/helper_bridge.rs:239-247 + main.rs:259)

The full execution chain: orchestrator -> control.ble ExecutePlan -> run_execute(helper, plan.kind) (e.g. "tap", "swipe", "scroll") -> helper CLI `execute --plan-kind tap` -> execute_pointer(&paths, "tap") (main.rs:124). In serve() (windows_hid.rs:331-378) the command kind is matched only against "stop" / "mouse" / "keyboard"; every other kind (which is what tap/swipe/scroll/etc. are) falls into the `_ => match server.execute_pointer_demo()` arm (line 368). execute_pointer_demo (line 546-559) sends pointer_demo_reports() — a fixed sequence of an 18px right move x8, 18px down move x8, left click, release.

Failure scenario: orchestrator sends plan.kind = "tap" (or any non-mouse/non-keyboard kind). iOS instead receives a fixed 18-pixel mouse wiggle + left click, the actual intended action is discarded, and the host reports phase "Succeeded" with observed_change true (main.rs:128-133). This is a correctness defect in the control execution path, not just a missing feature, because the `_` arm constructs and sends a (wrong) HID report sequence instead of erroring "unsupported plan kind".

### BUG-003 — ble-helper HID server deletes the command file before executing it, and an ack-write failure aborts the server and silently loses the command
File: helpers/ble-helper/src/windows_hid.rs:321-384

In serve(), for each pending command the file is read (line 322) and then immediately removed (line 323, `let _ = fs::remove_file(&command_path)`) BEFORE execute_mouse/execute_keyboard/execute_pointer_demo runs. Only later is the ack written (line 379, `write_ack(&paths, &command.id, &ack)?`), and the `?` propagates any filesystem error out of serve(), terminating the long-running HID server.

Failure scenario: a transient disk/permission error on write_ack (or any error from it) makes serve() return Err, killing the server. The caller (enqueue_hid_command, line 776-784) is polling the ack file for up to 5s; with the server dead and no ack written, it times out. Meanwhile the command file was already deleted at line 323, so the user's input event is gone forever — never executed, never acknowledged, never retryable. Fix: write the ack (or move the command to a "done"/"failed" queue) before deleting the command, and don't let an ack-write error kill the server.

### BUG-004 — WinRT GATT write handler blocks on GetRequestAsync().join() inside the event callback, risking STA re-entrancy/deadlock
File: helpers/ble-helper/src/windows_hid.rs:661-675

create_ack_write_characteristic installs a TypedEventHandler for the Protocol Mode (and HID Control Point) characteristic. Inside the handler it calls `args.GetRequestAsync()?.join()?` (line 668). The windows crate's IAsyncOperation::join() blocks the calling thread (pumping the STA message loop) until the operation completes. The handler itself is dispatched on that same STA thread.

Failure scenario: iOS writes to the Protocol Mode / HID Control Point characteristic; the handler runs on the STA, calls .join() which pumps messages, and a second write (or a state-change callback) is re-entered before the first deferral (line 667) completes, producing a deadlock or an "object is being used by another thread" error. The `?` then propagates an Error out of the handler, which the WinRT runtime treats as a failed callback, killing the write path. On iOS this manifests as the Protocol Mode / Control Point write hanging or silently failing.

## Medium severity

### BUG-005 — A multi-report HID command can exceed the 5s caller timeout while the server keeps sending, producing a wrong "timed out" result and orphaned ack files
File: helpers/ble-helper/src/windows_hid.rs:754-785 (caller), 561-580 (server)

enqueue_hid_command waits up to 5s (3s for stop) for the ack file (line 776-784). On timeout it returns Err WITHOUT removing the (not-yet-written) ack file. The server's execute_mouse (line 571-579) loops reports x repeat with thread::sleep(delay) per notify; a valid `--repeat 1000 --delay-ms 40` (parse_repeat_arg allows up to 1000, parse_delay_arg up to 1000) takes ~40s. The caller times out at 5s and reports "timed out waiting for BLE HID server response", but the server thread keeps sleeping+notifying and writes the ack ~35s later. By then the caller has moved on, so the ack file at acks_dir/{id}.json is never read or cleaned (only the success path at line 778 removes it).

Failure scenario: a long mouse/keyboard macro (high repeat + delay) is always reported as "timed out" even though it eventually executes on iOS, and ack files accumulate in the acks directory forever (resource leak + misleading failure). Fix: size the timeout to the command's repeat x delay, or have the caller clean up the ack file on timeout, or have the server refuse/queue commands longer than the timeout.

### BUG-006 — Windows atomic preferences save can delete the existing file then fail to rename, permanently losing all preferences
File: apps/host-desktop/src/preferences.rs:154-165

On Windows, if `std::fs::rename(&tmp_path, &self.path)` fails (e.g. target locked by another process / antivirus), the code enters the #[cfg(target_os = "windows")] block, sees self.path.exists() is true, calls `std::fs::remove_file(&self.path)?` (line 158) which succeeds — deleting the original preferences file — then calls `std::fs::rename(&tmp_path, &self.path)?` (line 159) which can fail again. The `?` propagates the error and the original preferences file is now gone with no recovery; the temp file also leaks (no cleanup on this second-failure path).

Failure scenario: an antivirus scanner briefly locks host-preferences.json during a save. The first rename fails, the code deletes the real file, the second rename also fails, and the user silently loses all saved preferences (selected device, source, BLE pointer scale, preview config, known-devices history). Fix: write to temp, and only remove the original AFTER the rename succeeds, or rename the original aside rather than deleting it first.

## Low severity

### BUG-007 — control.ble helper_bridge run_for_output leaks the stdout-drainer JoinHandle on the timeout/error path
File: plugins/control-ble/src/helper_bridge.rs:188-211

run_for_output spawns a drainer thread (line 198) that reads stdout until newline, then waits for the child via wait_for_completion (line 206). wait_for_completion returns Err on timeout (line 176-182), and that `?` at line 206 returns from run_for_output BEFORE the `stdout_handle.join()` at lines 207-209 is reached. The JoinHandle is dropped without being joined.

Failure scenario: every BLE helper RPC (probe/prepare/execute/status/stop/forget-bond) that times out leaks one JoinHandle. The child is killed+waited (line 177-178), so the OS pipe closes and the drainer thread's read_until returns EOF and the thread self-terminates shortly after — so it is a transient handle leak, not a permanent hang. Still, on rapid repeated timeouts the handles can pile up briefly. Fix: join (or detach with a result channel) the drainer on the error path too.

### BUG-008 — Closing the session window via the OS does not stop the runtime session, leaving it running invisibly
File: apps/host-desktop/src/app.rs:1548-1555

The session viewport window's close_requested handler sets session_window_open = false and clears window flags but never calls self.stop_session() / host_runtime.stop_session(device_id). The runtime_statuses still contain the active session, so poll_runtime_refresh_if_due keeps refreshing it and forward_preview_input keeps routing to it.

Failure scenario: the user starts a session, then closes the session window via the OS title-bar close button. The capture/control/grounding plugin subprocesses keep running invisibly (consuming the AirPlay mirror, BLE HID, etc.) with no visible window. The only way to stop it is to re-open the session window to reach the Stop button — which can_stop() may not even enable if the substate left Streaming (see BUG-009). Fix: call stop_session on close_requested (or at least surface the orphaned session in the dashboard so the user can stop it).

### BUG-009 — A Recovering session maps to SessionViewModel::starting(), whose can_stop() is false, so the user cannot stop a stuck recovering session from the UI
Files: apps/host-desktop/src/app.rs:1029-1040, apps/host-desktop/src/view_models/session.rs can_stop()

sync_selected_workspace matches SessionSubstate::Recovering in the Discovering | StartingCapture | StartingControl | Recovering arm (line 1029-1032), producing SessionViewModel::starting(). can_stop() returns true only for WaitingForMirror | Streaming, so the Stop button is disabled during recovery.

Failure scenario: a streaming session hits a transient capture/control failure and transitions to Recovering; if recovery loops indefinitely (e.g. the device was unplugged), the user has no UI way to stop/clean up the session and must restart the app. Fix: allow stopping a Recovering session (can_stop() should include the starting/recovering substates).

### BUG-010 — process_exists uses OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION), which succeeds for recycled PIDs, so a stale PID file pointing at an unrelated process prevents the HID server from restarting
File: helpers/ble-helper/src/windows_hid.rs:805-818

process_exists(pid) returns true for any PID OpenProcess succeeds on, including a recycled PID owned by an unrelated process. start_server_if_needed (line 233) treats a true result as "server already running" and returns Ok without spawning.

Failure scenario: the previous `ble-helper serve` crashed; Windows recycled its PID to an unrelated process. The next control call reads the stale pid_file, process_exists returns true, start_server_if_needed skips spawning a new server and returns Ok, and all subsequent mouse/keyboard/execute calls time out waiting for an ack that no server will ever write. Fix: validate the PID actually belongs to a ble-helper serve process (check the executable name/path) before trusting it.

## Investigated and rejected (not bugs)

- crates/session-orchestrator retry counter `attempts: u8` (lib.rs:657) does NOT overflow: RecoveryController::next_action (grounding-core/recovery_controller.rs:9-16) only returns Retry once (retries_used 0->1), so the execute loop is bounded at 2 attempts. The earlier "u8 retry overflow" suspicion was wrong.
- hid-report-engine expand_text_entry (hid-report-engine/src/lib.rs:3-24) only maps 'A' and 'b' and silently drops everything else via a `_ => usage_id: 0` arm — BUT it has no non-test callers. The real type-text path uses hid_key_for_ascii (helpers/ble-helper/src/main.rs:525+), which handles full ASCII and returns an error for unsupported characters. So the "silent text-entry drop" is dead test-only code, not a runtime defect.
- crates/plugin-runtime PluginRuntime::handshake (plugin-runtime/src/lib.rs:117) discards the spawned child on every path (including success). This is intentional: it is only used by inventory/providers/mod.rs and bootstrap/capability_probe.rs for capability probing, where the descriptor is wanted but the process is not. Not a bug.
- crates/frame-transport base64 decoder, frame-slot read/write, and Drop: re-verified correct (padding sentinel 64 guarded everywhere; bounds-checked copy_from_slice; reader re-checks mmap length). No bug.
- contracts/plugin-protocol/capability-registry/device-registry/telemetry-store: no verified defects. Roundtrips, enum shapes, and in-memory stores are consistent.
- plugins/capture-window, control-window-bridge, grounding-core, mock-device: no verified defects.

## Summary

10 verified bugs: 4 High (BUG-001..004), 2 Medium (BUG-005..006), 4 Low (BUG-007..010). The most impactful are the capture-session-replace ordering (BUG-001, loses a working stream on reconnect) and the BLE execute kind-collapse (BUG-002, every non-mouse/non-keyboard plan silently becomes a mouse wiggle while reporting success).
