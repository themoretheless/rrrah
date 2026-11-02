import ast
import json
import os
import subprocess
import sys
import tempfile
import unittest
import weakref
from pathlib import Path

from minhash_dedup.checkpoints import STAGE_OUTPUTS, StageCheckpoints
from minhash_dedup.runtime import Document


class TestStageCheckpoints(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.state = StageCheckpoints(self.root, {"plan": "fixture"})

    def complete_pipeline(self):
        for stage in STAGE_OUTPUTS:
            self.state.begin(stage)
            self.state.complete(stage)

    def test_one_empty_bucket_file_is_not_completion(self):
        (self.root / "buckets").mkdir()
        (self.root / "buckets" / "000_00.dups").touch()
        with self.assertRaises(FileNotFoundError):
            self.state.require_complete("2")
        self.state.begin("2", ["buckets/000_00.dups", "buckets/001_00.dups"])
        (self.root / "buckets").mkdir()
        (self.root / "buckets" / "000_00.dups").touch()
        with self.assertRaisesRegex(RuntimeError, "001_00"):
            self.state.complete("2")
        with self.assertRaises(FileNotFoundError):
            self.state.require_complete("2")
        (self.root / "buckets" / "001_00.dups").touch()
        self.state.complete("2")
        self.state.require_complete("2")

    def test_successful_empty_outputs_are_complete(self):
        self.complete_pipeline()
        self.state.require_complete("4")
        self.assertFalse((self.root / "removed").exists())

    def test_output_changes_and_corrupt_records_fail_closed(self):
        for mutation in ("truncate", "delete", "extra", "corrupt"):
            with self.subTest(mutation=mutation):
                self.state.begin("2")
                directory = self.root / "buckets"
                directory.mkdir()
                path = directory / "000_00.dups"
                path.write_bytes(b"payload")
                self.state.complete("2")
                if mutation == "truncate":
                    path.write_bytes(b"")
                elif mutation == "delete":
                    path.unlink()
                elif mutation == "extra":
                    (directory / "001_00.dups").touch()
                else:
                    self.state._path("2").write_text("{")
                with self.assertRaises((RuntimeError, json.JSONDecodeError)):
                    self.state.require_complete("2")

    def test_retry_cleans_only_own_outputs_and_invalidates_dependents(self):
        self.complete_pipeline()
        for name in ("buckets", "deduped", "log"):
            (self.root / name).mkdir()
            (self.root / name / "previous").touch()
        self.state.begin("2")
        self.assertFalse((self.root / "buckets").exists())
        self.assertTrue((self.root / "deduped" / "previous").exists())
        self.assertTrue((self.root / "log" / "previous").exists())
        for stage in STAGE_OUTPUTS:
            with self.assertRaises(FileNotFoundError):
                self.state.require_complete(stage)

    def test_configuration_and_upstream_generations_are_checked(self):
        self.complete_pipeline()
        old_child = self.state._path("2.5").read_bytes()
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            StageCheckpoints(self.root, {"plan": "different"}).require_complete("2")
        self.state.begin("2")
        self.state.complete("2")
        self.state._path("2.5").write_bytes(old_child)
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            self.state.require_complete("2.5")

    def test_stage_specific_options_do_not_invalidate_earlier_stages(self):
        self.complete_pipeline()
        changed = StageCheckpoints(self.root, {"plan": "fixture"}, {"4": {"data_labels": ["new"]}})
        changed.require_complete("3")
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            changed.require_complete("4")

    def test_signature_retry_clears_rank_and_downstream_completion(self):
        self.complete_pipeline()
        markers = self.root / "signatures" / "stage1_complete"
        markers.mkdir(parents=True)
        (markers / "00000.json").write_text("{}")
        self.state.invalidate("1")
        self.assertFalse(markers.exists())
        for stage in STAGE_OUTPUTS:
            self.assertFalse(self.state._path(stage).exists())

    def test_symlinked_output_directory_is_not_cleared(self):
        target = self.root / "unrelated"
        target.mkdir()
        (target / "keep").touch()
        (self.root / "buckets").symlink_to(target, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlinked"):
            self.state.begin("2")
        self.assertTrue((target / "keep").exists())


class TestStageLaunchers(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.scripts = Path(__file__).resolve().parents[1] / "scripts"
        self.checkout = self.root / "minhash_dedup" / "scripts"
        self.checkout.mkdir(parents=True)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        conda = self.root / "conda" / "etc" / "profile.d"
        conda.mkdir(parents=True)
        (conda / "conda.sh").write_text("conda() { return 0; }\n")
        self.executable("conda", '#!/bin/sh\nprintf "%s\\n" "$TEST_CONDA_BASE"\n')
        self.executable("activate", "# sourced environment stub\n:")
        self.executable("python3", f'#!/bin/sh\nexec "{sys.executable}" "$@"\n')
        self.executable("srun", '#!/bin/sh\nexit "$TEST_SRUN_EXIT"\n')
        self.events = self.root / "events"
        self.environment = os.environ | {
            "HOME": str(self.root),
            "PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
            "TEST_CONDA_BASE": str(self.root / "conda"),
            "TEST_EVENTS": str(self.events),
        }

    def executable(self, name, content):
        path = self.bin / name
        path.write_text(content)
        path.chmod(0o755)

    def test_wrappers_publish_only_after_successful_srun(self):
        helper = (
            "import os, sys\n"
            "with open(os.environ['TEST_EVENTS'], 'a') as stream:\n"
            "    stream.write(' '.join(sys.argv[1:]) + '\\n')\n"
        )
        (self.checkout / "minhash_stage_state.py").write_text(helper)
        (self.checkout / "minhash_stage1.py").write_text(helper)
        for stage in ("1", *STAGE_OUTPUTS):
            for code in (0, 17):
                with self.subTest(stage=stage, srun_exit=code):
                    self.events.write_text("")
                    result = subprocess.run(
                        ["bash", str(self.scripts / f"minhash_s{stage}.sh")],
                        env=self.environment | {"TEST_SRUN_EXIT": str(code)},
                        capture_output=True,
                        text=True,
                        timeout=30,
                    )
                    self.assertEqual(result.returncode, code, result.stdout + result.stderr)
                    events = self.events.read_text().splitlines()
                    if stage == "1":
                        self.assertEqual(events[:2], ["--prepare-only", "invalidate 1"])
                        self.assertEqual("--check-complete" in events, code == 0)
                    else:
                        self.assertEqual(events[0], f"begin {stage}")
                        self.assertEqual(f"complete {stage}" in events, code == 0)

    def test_driver_resubmits_all_dependents_after_first_incomplete_stage(self):
        # Run the production submission logic without its site-specific study exports.
        source = (self.scripts / "minhash_driver.sh").read_text()
        driver = self.checkout / "minhash_driver.sh"
        driver.write_text("#!/bin/bash\nset -euo pipefail\n" + source[source.index("missing_vars=()") :])
        for stage in ("1", *STAGE_OUTPUTS):
            (self.checkout / f"minhash_s{stage}.sh").touch()
        self.executable(
            "python3",
            """#!/bin/sh
if [ "$2" = "--check-complete" ]; then stage=1; else stage="$3"; fi
printf 'check %s\n' "$stage" >> "$TEST_EVENTS"
[ "$stage" != "$TEST_INCOMPLETE_STAGE" ]
""",
        )
        self.executable(
            "sbatch",
            """#!/bin/sh
printf 'submit %s\n' "$*" >> "$TEST_EVENTS"
printf '123\n'
""",
        )
        for incomplete in ("2", "none"):
            with self.subTest(incomplete=incomplete):
                self.events.write_text("")
                result = subprocess.run(
                    ["bash", str(driver)],
                    env=self.environment
                    | {
                        "TEST_INCOMPLETE_STAGE": incomplete,
                        "OUTPUT_PATH": str(self.root / "output"),
                        "S3_INPUT": '["input"]',
                        "DATA_LABEL": '["fixture"]',
                        "MINHASH_NUM_BUCKETS": "2",
                        "EXCLUDE_NODES": "",
                    },
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                lines = self.events.read_text().splitlines()
                submissions = [line for line in lines if line.startswith("submit ")]
                self.assertEqual(len(submissions), 4 if incomplete == "2" else 0)
                if submissions:
                    self.assertEqual([line for line in lines if line.startswith("check ")], ["check 1", "check 2"])
                    self.assertNotIn("--dependency", submissions[0])
                    for line in submissions[1:]:
                        self.assertIn("--dependency=afterok:123", line)

    def test_stage4_count_consumes_documents_without_retaining_them(self):
        # Exercise the actual counting expression without executing the stage CLI.
        tree = ast.parse((self.scripts / "minhash_stage4.py").read_text())
        assignment = next(
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Assign)
            and any(isinstance(target, ast.Name) and target.id == "filtered_count" for target in node.targets)
            and not isinstance(node.value, ast.Constant)
        )
        code = compile(ast.Module(body=[assignment], type_ignores=[]), "stage4-count", "exec")
        references = []

        def documents():
            for index in range(1000):
                references[:] = [reference for reference in references if reference() is not None]
                self.assertLessEqual(len(references), 2, "Counting retained the earlier documents")
                doc = Document("text", str(index))
                references.append(weakref.ref(doc))
                yield doc

        for data, expected in ((documents(), 1000), (iter(()), 0), (None, 0)):
            namespace = {"data": data}
            exec(code, namespace)
            self.assertEqual(namespace["filtered_count"], expected)


if __name__ == "__main__":
    unittest.main()
