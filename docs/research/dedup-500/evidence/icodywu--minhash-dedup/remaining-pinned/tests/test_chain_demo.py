import contextlib
import importlib.util
import io
import unittest
from pathlib import Path

DEMO = Path(__file__).resolve().parents[1] / "examples" / "chain_demo.py"


class TestChainDemo(unittest.TestCase):
    def test_bucket_feasible_selection_keeps_more_and_over_removes_less(self):
        spec = importlib.util.spec_from_file_location("chain_demo", DEMO)
        demo = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(demo)
        with contextlib.redirect_stdout(io.StringIO()):
            union_find, feasible = demo.main(["--chains", "10", "--exact-groups", "10", "--singletons", "20"])
        _, uf_kept, _, uf_over, _, _ = union_find
        _, kept, _, over, _, violations = feasible
        self.assertEqual(violations, 0)
        self.assertGreater(kept, uf_kept)
        self.assertLess(over, uf_over)


if __name__ == "__main__":
    unittest.main()
