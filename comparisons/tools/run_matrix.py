#!/usr/bin/env python3
"""Run and summarize the controlled JackVec comparison matrix."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import io
import json
import math
import os
import platform
import random
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

IMPLEMENTATIONS = ("Vec", "JackVec", "ThinVec", "SmallVec4", "SmallVec8")
PRACTICAL_BAND = (0.97, 1.03)
ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "comparisons" / "benchmark-results"
CRITERION = ROOT / "target" / "criterion"


def command(args: list[str], *, capture: bool = True, check: bool = True, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        args,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        env=env,
    )
    return completed.stdout.strip() if capture else ""


def git_dirty() -> bool:
    return bool(command(["git", "status", "--porcelain"]))


def host_metadata(cpu: int | None) -> dict[str, Any]:
    def optional(args: list[str]) -> str:
        try:
            return command(args)
        except (OSError, subprocess.CalledProcessError):
            return "unavailable"

    metadata: dict[str, Any] = {
        "timestamp_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "git_commit": command(["git", "rev-parse", "HEAD"]),
        "git_dirty": git_dirty(),
        "rustc": command(["rustc", "--version", "--verbose"]),
        "cargo": command(["cargo", "--version"]),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "logical_cpus": os.cpu_count(),
        "load_average": list(os.getloadavg()) if hasattr(os, "getloadavg") else None,
        "affinity_cpu": cpu,
    }
    if sys.platform == "darwin":
        metadata.update(
            {
                "cpu_model": optional(["sysctl", "-n", "machdep.cpu.brand_string"]),
                "memory_bytes": optional(["sysctl", "-n", "hw.memsize"]),
                "low_power_mode": optional(["pmset", "-g", "custom"]),
            }
        )
    elif sys.platform.startswith("linux"):
        metadata.update(
            {
                "cpu_summary": optional(["lscpu"]),
                "kernel": optional(["uname", "-a"]),
                "governor": optional(
                    ["cat", f"/sys/devices/system/cpu/cpu{cpu or 0}/cpufreq/scaling_governor"]
                ),
            }
        )
    return metadata


def host_issues(metadata: dict[str, Any]) -> list[str]:
    issues = []
    load = metadata.get("load_average")
    logical_cpus = metadata.get("logical_cpus") or 1
    if load and load[0] > logical_cpus * 0.5:
        issues.append(f"one-minute load average {load[0]:.2f} exceeds half of {logical_cpus} logical CPUs")
    if sys.platform.startswith("linux"):
        if metadata.get("affinity_cpu") is None:
            issues.append("Linux authoritative runs require --cpu affinity")
        governor = metadata.get("governor", "").strip()
        if governor != "performance":
            issues.append(f"CPU governor is {governor!r}, expected 'performance'")
    if sys.platform == "darwin" and "lowpowermode 1" in metadata.get("low_power_mode", ""):
        issues.append("macOS Low Power Mode is enabled")
    return issues


def benchmark_identity(estimates_file: Path) -> tuple[str, str]:
    relative = estimates_file.relative_to(CRITERION)
    parts = list(relative.parts[:-2])
    found = [part for part in parts if part in IMPLEMENTATIONS]
    if len(found) != 1:
        raise ValueError(f"expected exactly one implementation in {relative}, found {found}")
    implementation = found[0]
    parts.remove(implementation)
    return "/".join(parts), implementation


def read_criterion_round(started_ns: int) -> dict[str, dict[str, float]]:
    result: dict[str, dict[str, float]] = {}
    for path in CRITERION.glob("**/new/estimates.json"):
        if path.stat().st_mtime_ns + 1_000_000_000 < started_ns:
            continue
        benchmark, implementation = benchmark_identity(path)
        data = json.loads(path.read_text())
        estimate = float(data["median"]["point_estimate"])
        result.setdefault(benchmark, {})[implementation] = estimate
    validate_round(result)
    return result


def validate_round(result: dict[str, dict[str, float]]) -> None:
    if not result:
        raise ValueError("Criterion produced no fresh estimates")
    expected = set(IMPLEMENTATIONS)
    incomplete = {name: sorted(expected - set(values)) for name, values in result.items() if set(values) != expected}
    if incomplete:
        raise ValueError(f"incomplete comparison matrix: {incomplete}")


def percentile(values: list[float], probability: float) -> float:
    ordered = sorted(values)
    position = probability * (len(ordered) - 1)
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] * (1 - fraction) + ordered[upper] * fraction


def ratio_interval(ratios: list[float], samples: int = 10_000) -> tuple[float, float]:
    rng = random.Random(0x4A41434B)
    medians = []
    for _ in range(samples):
        medians.append(statistics.median(rng.choice(ratios) for _ in ratios))
    return percentile(medians, 0.025), percentile(medians, 0.975)


def classify(lower: float, upper: float) -> str:
    practical_low, practical_high = PRACTICAL_BAND
    if upper < practical_low:
        return "win"
    if lower > practical_high:
        return "loss"
    if lower >= practical_low and upper <= practical_high:
        return "equivalent"
    return "inconclusive"


def summarize(rounds: list[dict[str, dict[str, float]]]) -> list[dict[str, Any]]:
    benchmark_names = set(rounds[0])
    if any(set(round_) != benchmark_names for round_ in rounds):
        raise ValueError("benchmark identities changed between rounds")
    rows = []
    for benchmark in sorted(benchmark_names):
        vec = [round_[benchmark]["Vec"] for round_ in rounds]
        for implementation in IMPLEMENTATIONS:
            estimates = [round_[benchmark][implementation] for round_ in rounds]
            ratios = [estimate / baseline for estimate, baseline in zip(estimates, vec)]
            lower, upper = ratio_interval(ratios)
            rows.append(
                {
                    "benchmark": benchmark,
                    "implementation": implementation,
                    "median_ns": statistics.median(estimates),
                    "ratio_to_vec": statistics.median(ratios),
                    "ratio_ci95": [lower, upper],
                    "classification": "baseline" if implementation == "Vec" else classify(lower, upper),
                }
            )
    return rows


def markdown_report(document: dict[str, Any]) -> str:
    metadata = document["metadata"]
    lines = [
        f"# JackVec comparison: {document['platform_id']}",
        "",
        f"Commit: `{metadata['git_commit']}`  ",
        f"Rust: `{metadata['rustc'].splitlines()[0]}`  ",
        f"Platform: `{metadata['platform']}`  ",
        f"Rounds: {document['round_count']}; practical-equivalence band: 0.97–1.03× Vec.",
        "",
        "A win or loss requires the complete paired bootstrap interval to clear the practical-equivalence band. Results that cross a boundary are reported as inconclusive.",
        "",
        "## CPU",
        "",
        "| Benchmark | Implementation | Median ns | Ratio | 95% interval | Result |",
        "|---|---:|---:|---:|---:|---|",
    ]
    for row in document["cpu"]:
        low, high = row["ratio_ci95"]
        lines.append(
            f"| {row['benchmark']} | {row['implementation']} | {row['median_ns']:.3f} | "
            f"{row['ratio_to_vec']:.3f}× | {low:.3f}–{high:.3f}× | {row['classification']} |"
        )
    lines.extend(
        [
            "",
            "## Allocations",
            "",
            "Owner bytes describe the collection values themselves. Requested and usable bytes describe allocator-visible storage; they must not be added together for nested workloads.",
            "",
            "| Benchmark | Input | Element B | Implementation | Owner B | Live requested B | Live usable B | Allocs | Reallocs | Spilled |",
            "|---|---:|---:|---|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for row in document["allocations"]:
        lines.append(
            f"| {row['benchmark']} | {row['input']} | {row['element_size']} | {row['implementation']} | "
            f"{row['owner_bytes']} | {row['live_requested']} | {row['live_usable']} | "
            f"{row['allocations']} | {row['reallocations']} | {row['spilled_count']} |"
        )
    lines.append("")
    return "\n".join(lines)


def run(args: argparse.Namespace) -> None:
    dirty = git_dirty()
    if dirty and not args.allow_dirty:
        raise SystemExit("refusing an authoritative run from a dirty worktree; use --allow-dirty for exploratory data")
    if args.cpu is not None:
        if not hasattr(os, "sched_setaffinity"):
            raise SystemExit("--cpu is only supported on platforms with sched_setaffinity")
        os.sched_setaffinity(0, {args.cpu})

    metadata = host_metadata(args.cpu)
    issues = host_issues(metadata)
    if issues and not args.allow_host_noise:
        formatted = "\n- ".join(issues)
        raise SystemExit(f"refusing an authoritative run because:\n- {formatted}\nuse --allow-host-noise for exploratory data")
    authoritative = not dirty and not issues
    metadata["host_issues"] = issues
    metadata["authoritative"] = authoritative
    platform_id = args.output_name or f"{sys.platform}-{platform.machine().lower()}"
    if not authoritative:
        platform_id = f"{platform_id}-exploratory"
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    raw_dir = RESULTS / "raw" / f"{platform_id}-{timestamp}"
    raw_dir.mkdir(parents=True, exist_ok=False)

    command(["cargo", "build", "--release", "-p", "jack-vec-comparisons", "--benches"], capture=False)
    rounds = []
    for round_index in range(args.rounds):
        print(f"CPU round {round_index + 1}/{args.rounds}, rotation {round_index % 5}", flush=True)
        environment = os.environ.copy()
        environment["JACK_VEC_BENCH_ROTATION"] = str(round_index % 5)
        started_ns = time.time_ns()
        command(
            ["cargo", "bench", "-p", "jack-vec-comparisons", "--bench", "cpu", "--", "--noplot"],
            capture=False,
            env=environment,
        )
        round_data = read_criterion_round(started_ns)
        rounds.append(round_data)
        (raw_dir / f"cpu-round-{round_index + 1}.json").write_text(json.dumps(round_data, indent=2, sort_keys=True) + "\n")

    allocation_csv = command(["cargo", "bench", "-p", "jack-vec-comparisons", "--bench", "allocations"])
    (raw_dir / "allocations.csv").write_text(allocation_csv + "\n")
    allocations = list(csv.DictReader(io.StringIO(allocation_csv)))
    if not allocations:
        raise ValueError("allocation benchmark produced no rows")

    document = {
        "schema_version": 1,
        "platform_id": platform_id,
        "round_count": args.rounds,
        "practical_equivalence_band": list(PRACTICAL_BAND),
        "metadata": metadata,
        "cpu": summarize(rounds),
        "allocations": allocations,
    }
    RESULTS.mkdir(parents=True, exist_ok=True)
    json_path = RESULTS / f"{platform_id}.json"
    markdown_path = RESULTS / f"{platform_id}.md"
    json_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    markdown_path.write_text(markdown_report(document))
    print(f"wrote {json_path.relative_to(ROOT)} and {markdown_path.relative_to(ROOT)}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--output-name")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--allow-host-noise", action="store_true")
    args = parser.parse_args()
    if args.rounds < 3:
        parser.error("--rounds must be at least 3")
    return args


if __name__ == "__main__":
    run(parse_args())
