import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("run_matrix.py")
SPEC = importlib.util.spec_from_file_location("run_matrix", MODULE_PATH)
run_matrix = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(run_matrix)


class ReportingTests(unittest.TestCase):
    def test_system_allocator_policy_clears_and_records_injection(self):
        old = os.environ.get("LD_PRELOAD")
        try:
            os.environ["LD_PRELOAD"] = "/tmp/liballocator.so"
            metadata = run_matrix.configure_allocator("system")
            self.assertEqual(metadata["inherited_environment"]["LD_PRELOAD"], "/tmp/liballocator.so")
            self.assertIsNone(metadata["effective_environment"]["LD_PRELOAD"])
            self.assertNotIn("LD_PRELOAD", os.environ)
        finally:
            if old is None:
                os.environ.pop("LD_PRELOAD", None)
            else:
                os.environ["LD_PRELOAD"] = old

    def test_classification_boundaries(self):
        self.assertEqual(run_matrix.classify(0.90, 0.96), "win")
        self.assertEqual(run_matrix.classify(1.04, 1.10), "loss")
        self.assertEqual(run_matrix.classify(0.98, 1.02), "equivalent")
        self.assertEqual(run_matrix.classify(0.96, 1.02), "inconclusive")

    def test_summary_preserves_losses_and_equal_results(self):
        rounds = []
        for _ in range(5):
            rounds.append(
                {
                    "operation/1024": {
                        "Vec": 100.0,
                        "JackVec": 80.0,
                        "ThinVec": 110.0,
                        "SmallVec4": 100.0,
                        "SmallVec8": 101.0,
                    }
                }
            )
        rows = {row["implementation"]: row for row in run_matrix.summarize(rounds)}
        self.assertEqual(rows["JackVec"]["classification"], "win")
        self.assertEqual(rows["ThinVec"]["classification"], "loss")
        self.assertEqual(rows["SmallVec4"]["classification"], "equivalent")

    def test_matrix_validation_rejects_missing_implementation(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            run_matrix.validate_round({"operation": {"Vec": 1.0}})

    def test_host_issues_require_linux_affinity_and_performance_governor(self):
        original = run_matrix.sys.platform
        try:
            run_matrix.sys.platform = "linux"
            issues = run_matrix.host_issues(
                {"load_average": [0.0, 0.0, 0.0], "logical_cpus": 32, "affinity_cpu": None, "governor": "schedutil"}
            )
            self.assertEqual(len(issues), 2)
        finally:
            run_matrix.sys.platform = original

    def test_benchmark_identity_removes_only_implementation(self):
        original = run_matrix.CRITERION
        try:
            run_matrix.CRITERION = Path("/tmp/criterion")
            path = Path("/tmp/criterion/build/JackVec/1024/new/estimates.json")
            self.assertEqual(run_matrix.benchmark_identity(path), ("build/1024", "JackVec"))
        finally:
            run_matrix.CRITERION = original

    def test_pair_validation_rejects_compiler_and_matrix_differences(self):
        base = {
            "schema_version": 1,
            "round_count": 5,
            "toolchain": "1.97.0",
            "metadata": {
                "authoritative": True,
                "git_commit": "abc",
                "compiler_identity": {"release": "1.97.0", "commit_hash": "hash", "LLVM_version": "22"},
                "allocator": {
                    "policy": "system",
                    "inherited_environment": {"LD_PRELOAD": None, "DYLD_INSERT_LIBRARIES": None},
                    "effective_environment": {"LD_PRELOAD": None, "DYLD_INSERT_LIBRARIES": None},
                },
            },
            "cpu": [{"benchmark": "build/4", "implementation": "Vec"}],
            "allocations": [{"benchmark": "build", "input": "4", "element_size": "8", "implementation": "Vec"}],
        }
        other = json.loads(json.dumps(base))
        self.assertEqual(run_matrix.pair_issues(base, other), [])
        other["metadata"]["compiler_identity"]["commit_hash"] = "other"
        other["cpu"] = []
        issues = run_matrix.pair_issues(base, other)
        self.assertIn("CPU matrices differ", issues)
        self.assertTrue(any("commit_hash" in issue for issue in issues))


if __name__ == "__main__":
    unittest.main()
