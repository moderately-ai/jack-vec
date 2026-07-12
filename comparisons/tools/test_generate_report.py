import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("generate_report.py")
SPEC = importlib.util.spec_from_file_location("generate_report", MODULE_PATH)
generate_report = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(generate_report)

RESULTS = MODULE_PATH.parent.parent / "benchmark-results"


class GenerateReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.linux = json.loads((RESULTS / "linux-x86_64.json").read_text())
        cls.macos = json.loads((RESULTS / "macos-aarch64.json").read_text())

    def test_combined_latest_keeps_platforms_separate(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "LATEST.md"
            generate_report.write_latest([self.linux, self.macos], output)
            text = output.read_text()
        self.assertIn("## `linux-x86_64`", text)
        self.assertIn("## `macos-aarch64`", text)
        self.assertIn("platform results are presented in\nseparate sections and are never pooled", text.lower())
        self.assertEqual(text.count("### CPU outcomes"), 2)
        self.assertNotIn("macOS remains pending", text)

    def test_pair_mismatch_is_rejected(self):
        mismatched = json.loads(json.dumps(self.macos))
        mismatched["metadata"]["git_commit"] = "different"
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "LATEST.md"
            with self.assertRaisesRegex(ValueError, "git commits differ"):
                generate_report.write_latest([self.linux, mismatched], output)
            self.assertFalse(output.exists())

    def test_duplicate_platform_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "LATEST.md"
            with self.assertRaisesRegex(ValueError, "must be unique"):
                generate_report.write_latest([self.linux, self.linux], output)

    def test_non_system_report_is_rejected(self):
        report = json.loads(json.dumps(self.macos))
        report["metadata"]["allocator"]["policy"] = "environment"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "system allocator policy"):
                generate_report.load_report(path)


if __name__ == "__main__":
    unittest.main()
