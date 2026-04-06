import importlib.util
import hashlib
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "package_release.py"
SPEC = importlib.util.spec_from_file_location("package_release", MODULE_PATH)
package_release = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(package_release)

EXPECTED_HOST_BINARY = "host-desktop"
EXPECTED_PLUGIN_BINARIES = [
    "plugin-control-ble",
    "plugin-control-window-bridge",
    "plugin-capture-window",
    "plugin-capture-direct",
    "plugin-grounding-core",
    "plugin-mock-device",
]
EXPECTED_HELPER_BINARIES = [
    "ble-helper",
    "direct-beacon",
]


class PackageReleaseTests(unittest.TestCase):
    def _write_fake_binaries(self, bin_dir: Path, target: str) -> None:
        host_name = package_release.executable_name(EXPECTED_HOST_BINARY, target)
        host_path = bin_dir / host_name
        host_path.write_text("host", encoding="utf-8")
        host_path.chmod(0o755)
        for plugin in EXPECTED_PLUGIN_BINARIES:
            plugin_name = package_release.executable_name(plugin, target)
            plugin_path = bin_dir / plugin_name
            plugin_path.write_text(plugin, encoding="utf-8")
            plugin_path.chmod(0o755)
        for helper in EXPECTED_HELPER_BINARIES:
            helper_name = package_release.executable_name(helper, target)
            helper_path = bin_dir / helper_name
            helper_path.write_text(helper, encoding="utf-8")
            helper_path.chmod(0o755)

    def test_linux_bundle_and_plugin_archive(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            result = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="abc123",
                ref_name="refs/tags/v1.2.3",
                run_number="77",
                timestamp="2026-04-02T00:00:00Z",
            )

            self.assertEqual(result["bundle_archive"].suffixes[-2:], [".tar", ".gz"])
            self.assertEqual(result["plugin_archive"].suffixes[-2:], [".tar", ".gz"])

            bundle_root = f"ios-control-{target}"
            plugin_root = f"ios-control-plugins-{target}"
            with tarfile.open(result["bundle_archive"], "r:gz") as bundle_tar:
                bundle_names = bundle_tar.getnames()
                self.assertIn(
                    f"{bundle_root}/bin/{EXPECTED_HOST_BINARY}",
                    bundle_names,
                )
                host_member = bundle_tar.getmember(f"{bundle_root}/bin/{EXPECTED_HOST_BINARY}")
                self.assertEqual(stat.S_IMODE(host_member.mode), 0o755)
                for plugin in EXPECTED_PLUGIN_BINARIES:
                    self.assertIn(
                        f"{bundle_root}/plugins/{plugin}",
                        bundle_names,
                    )
                    plugin_member = bundle_tar.getmember(f"{bundle_root}/plugins/{plugin}")
                    self.assertEqual(stat.S_IMODE(plugin_member.mode), 0o755)
                for helper in EXPECTED_HELPER_BINARIES:
                    self.assertIn(
                        f"{bundle_root}/helpers/{helper}",
                        bundle_names,
                    )
                    helper_member = bundle_tar.getmember(f"{bundle_root}/helpers/{helper}")
                    self.assertEqual(stat.S_IMODE(helper_member.mode), 0o755)
                manifest = bundle_tar.extractfile(f"{bundle_root}/manifest.txt")
                self.assertIsNotNone(manifest)
                manifest_text = manifest.read().decode("utf-8")
                self.assertIn("sha=abc123", manifest_text)
                self.assertIn("ref_name=refs/tags/v1.2.3", manifest_text)
                self.assertIn(f"target={target}", manifest_text)

            with tarfile.open(result["plugin_archive"], "r:gz") as plugin_tar:
                plugin_names = plugin_tar.getnames()
                for plugin in EXPECTED_PLUGIN_BINARIES:
                    self.assertIn(
                        f"{plugin_root}/plugins/{plugin}",
                        plugin_names,
                    )
                for helper in EXPECTED_HELPER_BINARIES:
                    self.assertIn(
                        f"{plugin_root}/helpers/{helper}",
                        plugin_names,
                    )
                self.assertNotIn(
                    f"{plugin_root}/bin/{EXPECTED_HOST_BINARY}",
                    plugin_names,
                )
                manifest = plugin_tar.extractfile(f"{plugin_root}/manifest.txt")
                self.assertIsNotNone(manifest)
                manifest_text = manifest.read().decode("utf-8")
                self.assertIn("run_number=77", manifest_text)
                self.assertIn("timestamp=2026-04-02T00:00:00Z", manifest_text)

    def test_windows_bundle_and_plugin_archive(self) -> None:
        target = "x86_64-pc-windows-msvc"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            result = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="def456",
                ref_name="refs/heads/main",
                run_number="88",
                timestamp="2026-04-02T01:02:03Z",
            )

            self.assertEqual(result["bundle_archive"].suffix, ".zip")
            self.assertEqual(result["plugin_archive"].suffix, ".zip")

            bundle_root = f"ios-control-{target}"
            plugin_root = f"ios-control-plugins-{target}"
            host_exe = package_release.executable_name(EXPECTED_HOST_BINARY, target)

            with zipfile.ZipFile(result["bundle_archive"]) as bundle_zip:
                bundle_names = bundle_zip.namelist()
                self.assertIn(f"{bundle_root}/bin/{host_exe}", bundle_names)
                self.assertEqual(
                    (bundle_zip.getinfo(f"{bundle_root}/bin/{host_exe}").external_attr >> 16) & 0o777,
                    0o755,
                )
                for plugin in EXPECTED_PLUGIN_BINARIES:
                    plugin_exe = package_release.executable_name(plugin, target)
                    self.assertIn(f"{bundle_root}/plugins/{plugin_exe}", bundle_names)
                    self.assertEqual(
                        (bundle_zip.getinfo(f"{bundle_root}/plugins/{plugin_exe}").external_attr >> 16) & 0o777,
                        0o755,
                    )
                for helper in EXPECTED_HELPER_BINARIES:
                    helper_exe = package_release.executable_name(helper, target)
                    self.assertIn(f"{bundle_root}/helpers/{helper_exe}", bundle_names)
                    self.assertEqual(
                        (bundle_zip.getinfo(f"{bundle_root}/helpers/{helper_exe}").external_attr >> 16) & 0o777,
                        0o755,
                    )
                self.assertIn(f"{bundle_root}/manifest.txt", bundle_names)

            with zipfile.ZipFile(result["plugin_archive"]) as plugin_zip:
                plugin_names = plugin_zip.namelist()
                for plugin in EXPECTED_PLUGIN_BINARIES:
                    plugin_exe = package_release.executable_name(plugin, target)
                    self.assertIn(f"{plugin_root}/plugins/{plugin_exe}", plugin_names)
                for helper in EXPECTED_HELPER_BINARIES:
                    helper_exe = package_release.executable_name(helper, target)
                    self.assertIn(f"{plugin_root}/helpers/{helper_exe}", plugin_names)
                self.assertNotIn(f"{plugin_root}/bin/{host_exe}", plugin_names)
                self.assertIn(f"{plugin_root}/manifest.txt", plugin_names)

    def test_linux_bundle_includes_direct_runtime_tree(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            runtime_dir = root / "runtime"
            direct_runtime = runtime_dir / "uxplay" / target
            bin_dir.mkdir()
            out_dir.mkdir()
            direct_runtime.mkdir(parents=True)
            self._write_fake_binaries(bin_dir, target)
            (direct_runtime / "manifest.json").write_text("{}", encoding="utf-8")
            uxplay = direct_runtime / "uxplay"
            uxplay.write_text("uxplay", encoding="utf-8")
            uxplay.chmod(0o755)

            result = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                runtime_dir=runtime_dir,
                sha="abc123",
                ref_name="refs/tags/v1.2.3",
                run_number="77",
                timestamp="2026-04-02T00:00:00Z",
            )

            bundle_root = f"ios-control-{target}"
            plugin_root = f"ios-control-plugins-{target}"
            runtime_manifest = f"runtime/uxplay/{target}/manifest.json"

            with tarfile.open(result["bundle_archive"], "r:gz") as bundle_tar:
                self.assertIn(f"{bundle_root}/{runtime_manifest}", bundle_tar.getnames())

            with tarfile.open(result["plugin_archive"], "r:gz") as plugin_tar:
                self.assertIn(f"{plugin_root}/{runtime_manifest}", plugin_tar.getnames())

    def test_bundle_includes_direct_beacon_helper(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)
            helper_name = package_release.executable_name("direct-beacon", target)
            helper_path = bin_dir / helper_name
            helper_path.write_text("direct-beacon", encoding="utf-8")
            helper_path.chmod(0o755)

            result = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="abc123",
                ref_name="refs/tags/v1.2.3",
                run_number="77",
                timestamp="2026-04-02T00:00:00Z",
            )

            bundle_root = f"ios-control-{target}"
            plugin_root = f"ios-control-plugins-{target}"
            with tarfile.open(result["bundle_archive"], "r:gz") as bundle_tar:
                self.assertIn(
                    f"{bundle_root}/helpers/{helper_name}",
                    bundle_tar.getnames(),
                )
            with tarfile.open(result["plugin_archive"], "r:gz") as plugin_tar:
                self.assertIn(
                    f"{plugin_root}/helpers/{helper_name}",
                    plugin_tar.getnames(),
                )

    def test_missing_binary_raises_file_not_found_error(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)
            missing_plugin = package_release.executable_name(
                EXPECTED_PLUGIN_BINARIES[-1],
                target,
            )
            (bin_dir / missing_plugin).unlink()

            with self.assertRaises(FileNotFoundError):
                package_release.build_release_bundle(
                    target=target,
                    bin_dir=bin_dir,
                    out_dir=out_dir,
                    sha="abc123",
                    ref_name="refs/heads/main",
                    run_number="99",
                    timestamp="2026-04-02T03:00:00Z",
                )
            self.assertFalse((out_dir / f"ios-control-{target}").exists())
            self.assertFalse((out_dir / f"ios-control-plugins-{target}").exists())
            self.assertFalse((out_dir / f"ios-control-{target}.tar.gz").exists())
            self.assertFalse((out_dir / f"ios-control-plugins-{target}.tar.gz").exists())

    def test_failed_rebuild_removes_stale_archives(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            first = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="oldsha",
                ref_name="refs/tags/v1.0.0",
                run_number="1",
                timestamp="2026-04-02T03:00:00Z",
            )
            self.assertTrue(first["bundle_archive"].exists())
            self.assertTrue(first["plugin_archive"].exists())

            missing_plugin = package_release.executable_name(
                EXPECTED_PLUGIN_BINARIES[0],
                target,
            )
            (bin_dir / missing_plugin).unlink()

            with self.assertRaises(FileNotFoundError):
                package_release.build_release_bundle(
                    target=target,
                    bin_dir=bin_dir,
                    out_dir=out_dir,
                    sha="newsha",
                    ref_name="refs/tags/v2.0.0",
                    run_number="2",
                    timestamp="2026-04-02T04:00:00Z",
                )

            self.assertFalse(first["bundle_archive"].exists())
            self.assertFalse(first["plugin_archive"].exists())

    def test_cli_prints_both_archive_paths(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            command = [
                sys.executable,
                str(MODULE_PATH),
                "--target",
                target,
                "--bin-dir",
                str(bin_dir),
                "--out-dir",
                str(out_dir),
                "--sha",
                "abc123",
                "--ref-name",
                "refs/heads/main",
                "--run-number",
                "314",
                "--timestamp",
                "2026-04-02T05:00:00Z",
            ]
            completed = subprocess.run(
                command,
                check=True,
                capture_output=True,
                text=True,
            )

            lines = [line for line in completed.stdout.splitlines() if line]
            self.assertEqual(len(lines), 2)
            self.assertTrue(lines[0].endswith(".tar.gz"))
            self.assertTrue(lines[1].endswith(".tar.gz"))
            self.assertTrue(Path(lines[0]).exists())
            self.assertTrue(Path(lines[1]).exists())

    def test_archive_outputs_are_atomic_across_pair(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            extension = package_release.archive_extension(target)
            bundle_archive = out_dir / f"ios-control-{target}{extension}"
            plugin_archive = out_dir / f"ios-control-plugins-{target}{extension}"
            calls = {"count": 0}
            real_write_archive = package_release._write_archive

            def failing_second_write(*, source_dir: Path, archive_path: Path, target: str) -> None:
                calls["count"] += 1
                if calls["count"] == 2:
                    raise RuntimeError("simulated second archive failure")
                real_write_archive(source_dir=source_dir, archive_path=archive_path, target=target)

            with mock.patch.object(package_release, "_write_archive", side_effect=failing_second_write):
                with self.assertRaises(RuntimeError):
                    package_release.build_release_bundle(
                        target=target,
                        bin_dir=bin_dir,
                        out_dir=out_dir,
                        sha="abc123",
                        ref_name="refs/heads/main",
                        run_number="404",
                        timestamp="2026-04-02T06:00:00Z",
                    )

            self.assertFalse(bundle_archive.exists())
            self.assertFalse(plugin_archive.exists())

    def test_identical_inputs_produce_identical_archive_hashes(self) -> None:
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            out_dir = root / "out"
            bin_dir.mkdir()
            out_dir.mkdir()
            self._write_fake_binaries(bin_dir, target)

            first = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="abc123",
                ref_name="refs/heads/main",
                run_number="1000",
                timestamp="2026-04-02T07:00:00Z",
            )

            first_bundle_hash = hashlib.sha256(first["bundle_archive"].read_bytes()).hexdigest()
            first_plugin_hash = hashlib.sha256(first["plugin_archive"].read_bytes()).hexdigest()

            second = package_release.build_release_bundle(
                target=target,
                bin_dir=bin_dir,
                out_dir=out_dir,
                sha="abc123",
                ref_name="refs/heads/main",
                run_number="1000",
                timestamp="2026-04-02T07:00:00Z",
            )

            second_bundle_hash = hashlib.sha256(second["bundle_archive"].read_bytes()).hexdigest()
            second_plugin_hash = hashlib.sha256(second["plugin_archive"].read_bytes()).hexdigest()

            self.assertEqual(first_bundle_hash, second_bundle_hash)
            self.assertEqual(first_plugin_hash, second_plugin_hash)


if __name__ == "__main__":
    unittest.main()
