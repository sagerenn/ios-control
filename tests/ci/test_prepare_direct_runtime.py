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


if __name__ == "__main__":
    unittest.main()
