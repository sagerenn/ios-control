#!/usr/bin/env python3

from __future__ import annotations

import argparse
import shutil
import tarfile
import zipfile
from pathlib import Path


HOST_BINARY = "host-desktop"
PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]


def executable_name(binary_name: str, target: str) -> str:
    if "windows" in target.lower():
        return f"{binary_name}.exe"
    return binary_name


def archive_extension(target: str) -> str:
    if "windows" in target.lower():
        return ".zip"
    return ".tar.gz"


def manifest_text(
    *,
    sha: str,
    ref_name: str,
    target: str,
    run_number: str,
    timestamp: str,
) -> str:
    return (
        f"sha={sha}\n"
        f"ref_name={ref_name}\n"
        f"target={target}\n"
        f"run_number={run_number}\n"
        f"timestamp={timestamp}\n"
    )


def _copy_binary(*, bin_dir: Path, staged_path: Path, binary_name: str, target: str) -> None:
    source = bin_dir / executable_name(binary_name, target)
    if not source.exists():
        raise FileNotFoundError(source)
    staged_path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, staged_path)


def _write_archive(*, source_dir: Path, archive_path: Path, target: str) -> None:
    if archive_path.exists():
        archive_path.unlink()

    if archive_extension(target) == ".zip":
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(source_dir.rglob("*")):
                if path.is_file():
                    archive.write(path, arcname=path.relative_to(source_dir.parent))
        return

    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(source_dir, arcname=source_dir.name)


def build_release_bundle(
    *,
    target: str,
    bin_dir: Path,
    out_dir: Path,
    sha: str,
    ref_name: str,
    run_number: str,
    timestamp: str,
) -> dict[str, Path]:
    bin_dir = Path(bin_dir)
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    bundle_root = out_dir / f"ios-control-{target}"
    plugin_root = out_dir / f"ios-control-plugins-{target}"
    extension = archive_extension(target)
    bundle_archive = out_dir / f"{bundle_root.name}{extension}"
    plugin_archive = out_dir / f"{plugin_root.name}{extension}"

    for archive in (bundle_archive, plugin_archive):
        if archive.exists():
            archive.unlink()

    for root in (bundle_root, plugin_root):
        if root.exists():
            shutil.rmtree(root)

    _copy_binary(
        bin_dir=bin_dir,
        staged_path=bundle_root / "bin" / executable_name(HOST_BINARY, target),
        binary_name=HOST_BINARY,
        target=target,
    )

    for plugin in PLUGIN_BINARIES:
        plugin_filename = executable_name(plugin, target)
        _copy_binary(
            bin_dir=bin_dir,
            staged_path=bundle_root / "plugins" / plugin_filename,
            binary_name=plugin,
            target=target,
        )
        _copy_binary(
            bin_dir=bin_dir,
            staged_path=plugin_root / "plugins" / plugin_filename,
            binary_name=plugin,
            target=target,
        )

    manifest = manifest_text(
        sha=sha,
        ref_name=ref_name,
        target=target,
        run_number=run_number,
        timestamp=timestamp,
    )
    (bundle_root / "manifest.txt").write_text(manifest, encoding="utf-8")
    (plugin_root / "manifest.txt").write_text(manifest, encoding="utf-8")

    _write_archive(source_dir=bundle_root, archive_path=bundle_archive, target=target)
    _write_archive(source_dir=plugin_root, archive_path=plugin_archive, target=target)

    return {
        "bundle_root": bundle_root,
        "plugin_root": plugin_root,
        "bundle_archive": bundle_archive,
        "plugin_archive": plugin_archive,
    }


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build release bundle archives.")
    parser.add_argument("--target", required=True)
    parser.add_argument("--bin-dir", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--ref-name", required=True)
    parser.add_argument("--run-number", required=True)
    parser.add_argument("--timestamp", required=True)
    return parser.parse_args()


def main() -> None:
    args = _parse_args()
    result = build_release_bundle(
        target=args.target,
        bin_dir=args.bin_dir,
        out_dir=args.out_dir,
        sha=args.sha,
        ref_name=args.ref_name,
        run_number=args.run_number,
        timestamp=args.timestamp,
    )
    print(result["bundle_archive"])
    print(result["plugin_archive"])


if __name__ == "__main__":
    main()
