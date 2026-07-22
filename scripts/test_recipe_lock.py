import unittest

from recipe_lock import package_record, semantic_digest


class RecipeLockTests(unittest.TestCase):
    def test_registry_and_git_records_are_canonical(self):
        registry = {("dep", "1.2.3", "registry+index"): "abc"}
        self.assertEqual(
            package_record(
                {"name": "dep", "version": "1.2.3", "source": "registry+index"},
                {"features": ["z", "a"]},
                registry,
            ),
            "dep\t1.2.3\tregistry+index\tabc\ta,z",
        )
        self.assertEqual(
            package_record(
                {"name": "gitdep", "version": "2.0.0", "source": "git+https://example/x#deadbeef"},
                {"features": []},
                {},
            ),
            "gitdep\t2.0.0\tgit+https://example/x#deadbeef\t-\t",
        )

    def test_path_dependency_fails_closed(self):
        with self.assertRaisesRegex(RuntimeError, "path dependency"):
            package_record(
                {"name": "local", "version": "0.1.0", "source": None},
                {"features": []},
                {},
            )

    def test_semantic_digest_is_order_sensitive_but_callers_sort_records(self):
        first = semantic_digest(["a", "b"])
        self.assertNotEqual(first, semantic_digest(["b", "a"]))
        self.assertEqual(first, semantic_digest(sorted(["b", "a"])))


if __name__ == "__main__":
    unittest.main()
