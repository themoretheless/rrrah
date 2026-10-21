import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "qualify_ai_upstream", Path(__file__).resolve().parents[1] / "qualify-ai-upstream.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RendererOutcomes(unittest.TestCase):
    def test_events_survive_log_tail_for_text_and_timeout_bytes(self):
        event = "ORDERED_SAMPLE_OUTCOME patches=323 complete=false work_spent=100000000 work_left=0"
        detail = "prefix\n" + event + "\n" + "noise\n" * 4000
        for value in [detail, detail.encode()]:
            with self.subTest(kind=type(value).__name__):
                row = {}
                module.attach_renderer_diagnostics(row, value)
                self.assertNotIn(event, row["renderer_stderr_tail"])
                self.assertEqual(len(row["renderer_stderr_tail"]), 16384)
                self.assertEqual(row["renderer_outcomes"], [{
                    "patches": 323, "complete": False,
                    "work_spent": 100000000, "work_left": 0}])

    def test_success_records_are_preserved_and_capped(self):
        detail = "\n".join(
            f"ORDERED_SAMPLE_OUTCOME patches={i} complete=true work_spent=17 work_left=99999983"
            for i in range(70))
        row = {}
        module.attach_renderer_diagnostics(row, detail)
        self.assertEqual(len(row["renderer_outcomes"]), 64)
        self.assertTrue(row["renderer_outcomes_truncated"])
        self.assertTrue(row["renderer_outcomes"][0]["complete"])
        self.assertEqual(row["renderer_outcomes"][-1]["patches"], 63)

    def test_empty_malformed_and_non_utf8_input_are_safe(self):
        for value in [None, b"", ""]:
            row = {}
            module.attach_renderer_diagnostics(row, value)
            self.assertEqual(row, {})
        row = {}
        module.attach_renderer_diagnostics(row, b"\xffORDERED_SAMPLE_OUTCOME patches=x complete=true work_spent=0 work_left=1\n")
        self.assertNotIn("renderer_outcomes", row)
        self.assertIn("\ufffd", row["renderer_stderr_tail"])


if __name__ == "__main__":
    unittest.main()
