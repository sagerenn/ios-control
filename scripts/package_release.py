#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import io
import stat
import shutil
import tarfile
import zipfile
from pathlib import Path


HOST_BINARY = "host-desktop"
PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-control-window-bridge",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]
HELPER_BINARIES = [
    "ble-helper",
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


def _archive_mode(path: Path) -> int:
    return stat.S_IMODE(path.stat().st_mode)


def _copy_tree(*, source: Path, staged_path: Path) -> None:
    if not source.exists():
        return
    shutil.copytree(source, staged_path, dirs_exist_ok=True)


def _write_archive(*, source_dir: Path, archive_path: Path, target: str) -> None:
    if archive_path.exists():
        archive_path.unlink()

    if archive_extension(target) == ".zip":
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(source_dir.rglob("*")):
                if path.is_file():
                    archive_name = path.relative_to(source_dir.parent).as_posix()
                    info = zipfile.ZipInfo(archive_name, date_time=(1980, 1, 1, 0, 0, 0))
                    info.compress_type = zipfile.ZIP_DEFLATED
                    info.create_system = 3
                    info.external_attr = _archive_mode(path) << 16
                    archive.writestr(info, path.read_bytes())
        return

    with archive_path.open("wb") as raw_file:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw_file, mtime=0) as gzip_file:
            with tarfile.open(fileobj=gzip_file, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for path in sorted(source_dir.rglob("*")):
                    if not path.is_file():
                        continue
                    archive_name = path.relative_to(source_dir.parent).as_posix()
                    data = path.read_bytes()
                    info = tarfile.TarInfo(name=archive_name)
                    info.size = len(data)
                    info.mode = _archive_mode(path)
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    archive.addfile(info, io.BytesIO(data))


def build_release_bundle(
    *,
    target: str,
    bin_dir: Path,
    out_dir: Path,
    runtime_dir: Path | None = None,
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
    bundle_temp_archive = out_dir / f"{bundle_root.name}{extension}.tmp"
    plugin_temp_archive = out_dir / f"{plugin_root.name}{extension}.tmp"

    for archive in (
        bundle_archive,
        plugin_archive,
        bundle_temp_archive,
        plugin_temp_archive,
    ):
        if archive.exists():
            archive.unlink()

    try:
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

        for helper in HELPER_BINARIES:
            helper_filename = executable_name(helper, target)
            _copy_binary(
                bin_dir=bin_dir,
                staged_path=bundle_root / "helpers" / helper_filename,
                binary_name=helper,
                target=target,
            )
            _copy_binary(
                bin_dir=bin_dir,
                staged_path=plugin_root / "helpers" / helper_filename,
                binary_name=helper,
                target=target,
            )

        if runtime_dir is not None:
            _copy_tree(source=Path(runtime_dir), staged_path=bundle_root / "runtime")
            _copy_tree(source=Path(runtime_dir), staged_path=plugin_root / "runtime")

        manifest = manifest_text(
            sha=sha,
            ref_name=ref_name,
            target=target,
            run_number=run_number,
            timestamp=timestamp,
        )
        bundle_manifest = bundle_root / "manifest.txt"
        plugin_manifest = plugin_root / "manifest.txt"
        bundle_manifest.write_text(manifest, encoding="utf-8")
        plugin_manifest.write_text(manifest, encoding="utf-8")
        bundle_manifest.chmod(0o644)
        plugin_manifest.chmod(0o644)

        _write_archive(source_dir=bundle_root, archive_path=bundle_temp_archive, target=target)
        _write_archive(source_dir=plugin_root, archive_path=plugin_temp_archive, target=target)
        bundle_temp_archive.replace(bundle_archive)
        plugin_temp_archive.replace(plugin_archive)
    except Exception:
        for root in (bundle_root, plugin_root):
            if root.exists():
                shutil.rmtree(root)
        for archive in (
            bundle_archive,
            plugin_archive,
            bundle_temp_archive,
            plugin_temp_archive,
        ):
            if archive.exists():
                archive.unlink()
        raise

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
    parser.add_argument("--runtime-dir", required=False, type=Path)
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
        runtime_dir=args.runtime_dir,
        sha=args.sha,
        ref_name=args.ref_name,
        run_number=args.run_number,
        timestamp=args.timestamp,
    )
    print(result["bundle_archive"])
    print(result["plugin_archive"])


if __name__ == "__main__":
    main()
