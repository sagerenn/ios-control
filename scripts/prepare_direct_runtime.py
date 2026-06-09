#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

WINDOWS_GSTREAMER_PLUGIN_EXCLUDES = {
    # This plugin pulls in libsoup/libpsl/libidn2. The direct AirPlay receiver
    # does not use HTTP sources, and partial Windows installs can otherwise load
    # an incompatible libidn2-0.dll from unrelated software on PATH.
    "libgstsoup.dll",
}


def executable_name(binary_name: str, target: str) -> str:
    if "windows" in target.lower():
        return f"{binary_name}.exe"
    return binary_name


def gst_launch_relpath(target: str) -> str:
    return f"gstreamer/bin/{executable_name('gst-launch-1.0', target)}"


def stage_direct_runtime(
    *,
    target: str,
    out_dir: Path,
    uxplay_path: Path,
    uxplay_support_paths: list[Path],
    gst_root: Path,
    beacon_script: Path,
    beacon_helper_relpath: str,
    python_path: str,
    uxplay_version: str,
    gstreamer_version: str,
) -> Path:
    target_root = Path(out_dir) / "uxplay" / target
    if target_root.exists():
        shutil.rmtree(target_root)
    target_root.mkdir(parents=True, exist_ok=True)

    staged_uxplay = target_root / executable_name("uxplay", target)
    shutil.copy2(uxplay_path, staged_uxplay)
    for support_path in uxplay_support_paths:
        shutil.copy2(support_path, target_root / support_path.name)

    staged_gst = target_root / "gstreamer"
    shutil.copytree(gst_root, staged_gst, dirs_exist_ok=True)
    if "windows" in target.lower():
        plugin_dir = staged_gst / "lib" / "gstreamer-1.0"
        for plugin_name in WINDOWS_GSTREAMER_PLUGIN_EXCLUDES:
            plugin_path = plugin_dir / plugin_name
            if plugin_path.exists():
                plugin_path.unlink()

    beacon_dir = target_root / "Bluetooth_LE_beacon"
    beacon_dir.mkdir(parents=True, exist_ok=True)
    for beacon_path in sorted(beacon_script.parent.glob("uxplay_beacon_module_*.py")):
        shutil.copy2(beacon_path, beacon_dir / beacon_path.name)
    shutil.copy2(beacon_script, beacon_dir / "uxplay-beacon.py")

    manifest = {
        "uxplay_path": staged_uxplay.name,
        "gst_launch_path": gst_launch_relpath(target),
        "beacon_helper_path": beacon_helper_relpath,
        "beacon_script_path": "Bluetooth_LE_beacon/uxplay-beacon.py",
        "python_path": python_path,
        "uxplay_version": uxplay_version,
        "gstreamer_version": gstreamer_version,
    }
    manifest_path = target_root / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8")
    return manifest_path


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Stage a direct runtime bundle tree.")
    parser.add_argument("--target", required=True)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--uxplay-path", required=True, type=Path)
    parser.add_argument(
        "--uxplay-support-path",
        dest="uxplay_support_paths",
        action="append",
        default=[],
        type=Path,
    )
    parser.add_argument("--gst-root", required=True, type=Path)
    parser.add_argument("--beacon-script", required=True, type=Path)
    parser.add_argument("--beacon-helper-relpath", required=True)
    parser.add_argument("--python-path", required=True)
    parser.add_argument("--uxplay-version", required=True)
    parser.add_argument("--gstreamer-version", required=True)
    return parser.parse_args()


def main() -> None:
    args = _parse_args()
    manifest_path = stage_direct_runtime(
        target=args.target,
        out_dir=args.out_dir,
        uxplay_path=args.uxplay_path,
        uxplay_support_paths=args.uxplay_support_paths,
        gst_root=args.gst_root,
        beacon_script=args.beacon_script,
        beacon_helper_relpath=args.beacon_helper_relpath,
        python_path=args.python_path,
        uxplay_version=args.uxplay_version,
        gstreamer_version=args.gstreamer_version,
    )
    print(manifest_path)


if __name__ == "__main__":
    main()
