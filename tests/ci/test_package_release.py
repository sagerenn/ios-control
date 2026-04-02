import importlib.util
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "package_release.py"
SPEC = importlib.util.spec_from_file_location("package_release", MODULE_PATH)
package_release = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(package_release)


class PackageReleaseTests(unittest.TestCase):
    def _write_fake_binaries(self, bin_dir: Path, target: str) -> None:
        host_name = package_release.executable_name(package_release.HOST_BINARY, target)
        (bin_dir / host_name).write_text("host", encoding="utf-8")
        for plugin in package_release.PLUGIN_BINARIES:
            plugin_name = package_release.executable_name(plugin, target)
            (bin_dir / plugin_name).write_text(plugin, encoding="utf-8")

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
                    f"{bundle_root}/bin/{package_release.HOST_BINARY}",
                    bundle_names,
                )
                self.assertIn(
                    f"{bundle_root}/plugins/{package_release.PLUGIN_BINARIES[0]}",
                    bundle_names,
                )
                manifest = bundle_tar.extractfile(f"{bundle_root}/manifest.txt")
                self.assertIsNotNone(manifest)
                manifest_text = manifest.read().decode("utf-8")
                self.assertIn("sha=abc123", manifest_text)
                self.assertIn("ref_name=refs/tags/v1.2.3", manifest_text)
                self.assertIn(f"target={target}", manifest_text)

            with tarfile.open(result["plugin_archive"], "r:gz") as plugin_tar:
                plugin_names = plugin_tar.getnames()
                self.assertIn(
                    f"{plugin_root}/plugins/{package_release.PLUGIN_BINARIES[0]}",
                    plugin_names,
                )
                self.assertNotIn(
                    f"{plugin_root}/bin/{package_release.HOST_BINARY}",
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
            host_exe = package_release.executable_name(package_release.HOST_BINARY, target)
            plugin_exe = package_release.executable_name(package_release.PLUGIN_BINARIES[0], target)

            with zipfile.ZipFile(result["bundle_archive"]) as bundle_zip:
                bundle_names = bundle_zip.namelist()
                self.assertIn(f"{bundle_root}/bin/{host_exe}", bundle_names)
                self.assertIn(f"{bundle_root}/plugins/{plugin_exe}", bundle_names)

            with zipfile.ZipFile(result["plugin_archive"]) as plugin_zip:
                plugin_names = plugin_zip.namelist()
                self.assertIn(f"{plugin_root}/plugins/{plugin_exe}", plugin_names)


if __name__ == "__main__":
    unittest.main()
