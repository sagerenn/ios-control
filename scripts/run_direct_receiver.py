#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path


def default_bundle_root(repo_root: Path) -> Path:
    return (
        repo_root
        / "dist"
        / "x86_64-pc-windows-msvc"
        / "ios-control-x86_64-pc-windows-msvc"
    )


def request(process: subprocess.Popen[str], payload: object) -> dict:
    assert process.stdin is not None
    assert process.stdout is not None
    process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
    process.stdin.flush()
    line = process.stdout.readline()
    if not line:
        raise RuntimeError("direct receiver plugin exited without a reply")
    return json.loads(line)


def drain_stderr(process: subprocess.Popen[str], log_file) -> None:
    assert process.stderr is not None
    for line in process.stderr:
        log_file.write(f"[stderr] {line}")
        log_file.flush()


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="Run the packaged direct screen receiver.")
    parser.add_argument("--bundle-root", type=Path, default=default_bundle_root(repo_root))
    parser.add_argument("--log", type=Path, default=repo_root / ".receiver" / "direct-receiver.log")
    parser.add_argument("--pid-file", type=Path, default=repo_root / ".receiver" / "direct-receiver.pid")
    parser.add_argument("--poll-seconds", type=float, default=2.0)
    args = parser.parse_args()

    bundle_root = args.bundle_root.resolve()
    plugin = bundle_root / "plugins" / "plugin-capture-direct.exe"
    runtime_root = bundle_root / "runtime" / "uxplay" / "x86_64-pc-windows-msvc"
    if not plugin.is_file():
        raise FileNotFoundError(plugin)
    if not (runtime_root / "manifest.json").is_file():
        raise FileNotFoundError(runtime_root / "manifest.json")

    args.log.parent.mkdir(parents=True, exist_ok=True)
    args.pid_file.parent.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["IOS_CONTROL_DIRECT_RUNTIME_ROOT"] = str(runtime_root)

    with args.log.open("a", encoding="utf-8") as log_file:
        log_file.write(f"\n=== direct receiver start {time.strftime('%Y-%m-%dT%H:%M:%S')} ===\n")
        log_file.write(f"bundle_root={bundle_root}\n")
        log_file.write(f"runtime_root={runtime_root}\n")
        log_file.flush()

        process = subprocess.Popen(
            [str(plugin)],
            cwd=str(bundle_root),
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            bufsize=1,
        )
        args.pid_file.write_text(str(process.pid), encoding="utf-8")
        threading.Thread(target=drain_stderr, args=(process, log_file), daemon=True).start()

        try:
            for payload in (
                {"Handshake": {"protocol_version": 3}},
                "ProbeCapture",
                {"OpenCaptureStream": {"source_id": "direct-1"}},
            ):
                reply = request(process, payload)
                log_file.write(json.dumps(reply, ensure_ascii=True) + "\n")
                log_file.flush()
                if "Error" in reply:
                    raise RuntimeError(reply["Error"].get("message", "direct receiver error"))

            print("Direct screen receiver is running.")
            print(f"Log: {args.log}")
            print(f"PID: {process.pid}")
            print("On iPhone: Control Center > Screen Mirroring, choose the UxPlay receiver.")

            while process.poll() is None:
                for payload in ("GetCaptureStatus", "ReadCaptureFrame"):
                    reply = request(process, payload)
                    log_file.write(json.dumps(reply, ensure_ascii=True) + "\n")
                    log_file.flush()
                time.sleep(args.poll_seconds)
        except KeyboardInterrupt:
            pass
        finally:
            if process.poll() is None:
                for payload in ("CloseCaptureStream", "Stop"):
                    try:
                        reply = request(process, payload)
                        log_file.write(json.dumps(reply, ensure_ascii=True) + "\n")
                    except Exception as error:
                        log_file.write(f"shutdown request failed: {error}\n")
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
            args.pid_file.unlink(missing_ok=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
