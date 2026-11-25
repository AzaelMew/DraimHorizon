"""Exercise the DEV bridges without a game or SDK, using the actual hook bodies."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SunriseDevBridgeTests(unittest.TestCase):
    def test_command_consumption_and_local_chat_trust(self):
        rustc = shutil.which("rustc")
        if rustc is None:
            self.skipTest("no rustc")
        source = (ROOT / "tools/tests/fixtures/sunrise_dev.rs").read_text(encoding="utf-8")
        overlay = ROOT / "platforms/sunrise/overlay/sunrise"
        source += (overlay / "command_hook.rs").read_text(encoding="utf-8")
        source += (overlay / "chat_capture_line_hook.rs").read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            path = temporary / "bridges.rs"
            path.write_text(source, encoding="utf-8")
            binary = temporary / "bridges"
            built = subprocess.run([rustc, "--edition", "2024", "--test", str(path), "-o", str(binary)],
                                   capture_output=True, text=True)
            self.assertEqual(built.returncode, 0, built.stderr)
            tested = subprocess.run([str(binary)], capture_output=True, text=True)
            self.assertEqual(tested.returncode, 0, tested.stdout + tested.stderr)
