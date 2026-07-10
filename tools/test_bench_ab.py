#!/usr/bin/env python3

import tempfile
from pathlib import Path
import unittest

import bench_ab


class BenchAbTests(unittest.TestCase):
    def test_balanced_order_is_deterministic_and_alternating(self):
        order = bench_ab.balanced_order(7, 42)
        self.assertEqual(order, bench_ab.balanced_order(7, 42))
        self.assertTrue(
            all(sorted(pair) == ["baseline", "candidate"] for pair in order)
        )
        self.assertTrue(
            all(order[index] != order[index + 1] for index in range(len(order) - 1))
        )

    def test_percentile_interpolates(self):
        self.assertEqual(bench_ab.percentile([0.0, 10.0], 0.25), 2.5)

    def test_environment_subset_omits_credentials(self):
        subset = bench_ab.environment_subset(
            {
                "LD_PRELOAD": "/allocator.so",
                "CARGO_PROFILE_BENCH_LTO": "true",
                "CARGO_REGISTRIES_PRIVATE_TOKEN": "secret",
            }
        )
        self.assertEqual(
            subset,
            {"LD_PRELOAD": "/allocator.so", "CARGO_PROFILE_BENCH_LTO": "true"},
        )

    def test_summary_uses_paired_ratios(self):
        rows = []
        for round_number, baseline, candidate in ((1, 100.0, 90.0), (2, 200.0, 220.0)):
            rows.extend(
                [
                    {
                        "round": round_number,
                        "label": "baseline",
                        "benchmark": "operation/ThinVec/4",
                        "mean_ns": baseline,
                    },
                    {
                        "round": round_number,
                        "label": "candidate",
                        "benchmark": "operation/ThinVec/4",
                        "mean_ns": candidate,
                    },
                ]
            )
        summary = bench_ab.summarize_measurements(rows, seed=1)[0]
        self.assertAlmostEqual(summary["median_delta_percent"], 0.0)
        self.assertEqual(summary["pairs"], 2)

    def test_collect_estimates_reads_each_round(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for round_number in (1, 2):
                for label in ("baseline", "candidate"):
                    estimate_dir = (
                        root
                        / "runs"
                        / f"{round_number:03d}-{label}"
                        / "criterion"
                        / "group"
                        / "ThinVec"
                        / "4"
                        / "new"
                    )
                    estimate_dir.mkdir(parents=True)
                    (estimate_dir / "estimates.json").write_text(
                        '{"mean":{"point_estimate":1.0},'
                        '"median":{"point_estimate":2.0},'
                        '"slope":{"point_estimate":3.0}}'
                    )
            rows = bench_ab.collect_estimates(root, 2)
            self.assertEqual(len(rows), 4)
            self.assertEqual(rows[0]["benchmark"], "group/ThinVec/4")


if __name__ == "__main__":
    unittest.main()
