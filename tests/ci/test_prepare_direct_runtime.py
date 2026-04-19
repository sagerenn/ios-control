import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts" / "prepare_direct_runtime.py"


class PrepareDirectRuntimeTests(unittest.TestCase):
    def test_prepare_direct_runtime_writes_manifest_and_stages_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            out_dir = root / "runtime"
            gst_root = root / "gst"
            beacon_script = root / "uxplay-beacon.py"
            uxplay = root / "uxplay"

            (gst_root / "bin").mkdir(parents=True)
            (gst_root / "bin" / "gst-launch-1.0").write_text("gst", encoding="utf-8")
            uxplay.write_text("uxplay", encoding="utf-8")
            beacon_script.write_text("print('ok')\n", encoding="utf-8")

            result = subprocess.run(
                [
                    sys.executable,
                    str(MODULE_PATH),
                    "--target",
                    "x86_64-unknown-linux-gnu",
                    "--out-dir",
                    str(out_dir),
                    "--uxplay-path",
                    str(uxplay),
                    "--gst-root",
                    str(gst_root),
                    "--beacon-script",
                    str(beacon_script),
                    "--beacon-helper-relpath",
                    "../../../helpers/direct-beacon",
                    "--python-path",
                    "python3",
                    "--uxplay-version",
                    "v1.73.6",
                    "--gstreamer-version",
                    "1.26.3",
                ],
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            target_root = out_dir / "uxplay" / "x86_64-unknown-linux-gnu"
            manifest_path = target_root / "manifest.json"
            self.assertTrue(manifest_path.exists())
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(manifest["uxplay_path"], "uxplay")
            self.assertEqual(manifest["gst_launch_path"], "gstreamer/bin/gst-launch-1.0")
            self.assertEqual(
                manifest["beacon_helper_path"], "../../../helpers/direct-beacon"
            )
            self.assertEqual(
                manifest["beacon_script_path"],
                "Bluetooth_LE_beacon/uxplay-beacon.py",
            )
            self.assertEqual(manifest["python_path"], "python3")
            self.assertTrue((target_root / "uxplay").exists())
            self.assertTrue((target_root / "gstreamer" / "bin" / "gst-launch-1.0").exists())
            self.assertTrue(
                (target_root / "Bluetooth_LE_beacon" / "uxplay-beacon.py").exists()
            )

    def test_prepare_direct_runtime_stages_windows_uxplay_support_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            out_dir = root / "runtime"
            gst_root = root / "gst"
            beacon_script = root / "uxplay-beacon.py"
            uxplay = root / "uxplay.exe"
            support_root = root / "support"
            libstdcpp = support_root / "libstdc++-6.dll"
            libplist = support_root / "libplist-2.0.dll"
            libgobject = support_root / "libgobject-2.0-0.dll"
            libgstapp = support_root / "libgstapp-1.0-0.dll"

            (gst_root / "bin").mkdir(parents=True)
            support_root.mkdir(parents=True)
            (gst_root / "bin" / "gst-launch-1.0.exe").write_text("gst", encoding="utf-8")
            uxplay.write_text("uxplay", encoding="utf-8")
            libstdcpp.write_text("libstdc++", encoding="utf-8")
            libplist.write_text("libplist", encoding="utf-8")
            libgobject.write_text("libgobject", encoding="utf-8")
            libgstapp.write_text("libgstapp", encoding="utf-8")
            beacon_script.write_text("print('ok')\n", encoding="utf-8")

            result = subprocess.run(
                [
                    sys.executable,
                    str(MODULE_PATH),
                    "--target",
                    "x86_64-pc-windows-msvc",
                    "--out-dir",
                    str(out_dir),
                    "--uxplay-path",
                    str(uxplay),
                    "--uxplay-support-path",
                    str(libstdcpp),
                    "--uxplay-support-path",
                    str(libplist),
                    "--uxplay-support-path",
                    str(libgobject),
                    "--uxplay-support-path",
                    str(libgstapp),
                    "--gst-root",
                    str(gst_root),
                    "--beacon-script",
                    str(beacon_script),
                    "--beacon-helper-relpath",
                    "../../../helpers/direct-beacon.exe",
                    "--python-path",
                    "python",
                    "--uxplay-version",
                    "v1.73.6",
                    "--gstreamer-version",
                    "1.26.3",
                ],
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            target_root = out_dir / "uxplay" / "x86_64-pc-windows-msvc"
            self.assertTrue((target_root / "uxplay.exe").exists())
            self.assertTrue((target_root / "libstdc++-6.dll").exists())
            self.assertTrue((target_root / "libplist-2.0.dll").exists())
            self.assertTrue((target_root / "libgobject-2.0-0.dll").exists())
            self.assertTrue((target_root / "libgstapp-1.0-0.dll").exists())


if __name__ == "__main__":
    unittest.main()
