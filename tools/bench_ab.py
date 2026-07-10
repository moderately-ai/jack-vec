#!/usr/bin/env python3
"""Run reproducible, paired Criterion A/B measurements for two Git commits."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any, Sequence


class RunnerError(RuntimeError):
    pass


def run_checked(
    command: Sequence[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    stdout: Any = subprocess.PIPE,
    stderr: Any = subprocess.PIPE,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=stdout,
        stderr=stderr,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() if isinstance(result.stderr, str) else ""
        raise RunnerError(
            f"command failed ({result.returncode}): {' '.join(command)}\n{detail}"
        )
    return result


def command_output(command: Sequence[str], cwd: Path) -> str:
    try:
        return run_checked(command, cwd=cwd).stdout.strip()
    except (OSError, RunnerError) as error:
        return f"unavailable: {error}"


def resolve_commit(repo: Path, reference: str) -> str:
    return run_checked(
        ["git", "rev-parse", "--verify", f"{reference}^{{commit}}"], cwd=repo
    ).stdout.strip()


def git_path_digest(repo: Path, commit: str, paths: Sequence[str]) -> str:
    digest = hashlib.sha256()
    for path in paths:
        listing = run_checked(
            ["git", "ls-tree", "-r", "--full-tree", commit, "--", path], cwd=repo
        ).stdout
        digest.update(path.encode())
        digest.update(b"\0")
        digest.update(listing.encode())
    return digest.hexdigest()


def balanced_order(rounds: int, seed: int) -> list[list[str]]:
    first = random.Random(seed).choice(("baseline", "candidate"))
    second = "candidate" if first == "baseline" else "baseline"
    return [
        [first, second] if index % 2 == 0 else [second, first]
        for index in range(rounds)
    ]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def cargo_command(toolchain: str, *arguments: str) -> list[str]:
    command = ["cargo"]
    if toolchain:
        command.append(f"+{toolchain}")
    command.extend(arguments)
    return command


def build_benchmark(
    worktree: Path,
    target_dir: Path,
    toolchain: str,
    bench: str,
    virtual_source_root: str,
    log_dir: Path,
    base_env: dict[str, str],
) -> Path:
    env = base_env.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    remap = f"--remap-path-prefix={worktree}={virtual_source_root}"
    env["RUSTFLAGS"] = " ".join(filter(None, (env.get("RUSTFLAGS", ""), remap)))
    command = cargo_command(
        toolchain,
        "bench",
        "--locked",
        "--bench",
        bench,
        "--no-run",
        "--message-format=json",
    )
    result = run_checked(command, cwd=worktree, env=env)
    (log_dir / "build.jsonl").write_text(result.stdout)
    (log_dir / "build.stderr.log").write_text(result.stderr)

    executables: list[Path] = []
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        if (
            message.get("reason") == "compiler-artifact"
            and target.get("name") == bench
            and "bench" in target.get("kind", [])
            and message.get("executable")
        ):
            executables.append(Path(message["executable"]))
    if len(executables) != 1:
        raise RunnerError(
            f"expected one {bench!r} benchmark executable, found {executables}"
        )
    return executables[0]


def run_benchmark(
    executable: Path,
    runtime_dir: Path,
    criterion_home: Path,
    run_dir: Path,
    args: argparse.Namespace,
    base_env: dict[str, str],
    source_label: str,
) -> None:
    criterion_home.mkdir(parents=True)
    # Cargo normally supplies the hidden --bench compatibility flag. Without it,
    # a directly invoked Criterion executable only performs its test-mode smoke run
    # and emits no measurements.
    command = [str(executable), "--bench", args.filter, "--noplot", "--color", "never"]
    if args.exact:
        command.append("--exact")
    command.extend(
        [
            "--sample-size",
            str(args.sample_size),
            "--warm-up-time",
            str(args.warm_up_time),
            "--measurement-time",
            str(args.measurement_time),
            "--nresamples",
            str(args.nresamples),
        ]
    )
    if args.cpu is not None:
        command = ["taskset", "-c", str(args.cpu), *command]

    env = base_env.copy()
    env["CRITERION_HOME"] = str(criterion_home)
    started = dt.datetime.now(dt.timezone.utc)
    monotonic_start = time.monotonic()
    with (
        (run_dir / "stdout.log").open("w") as stdout,
        (run_dir / "stderr.log").open("w") as stderr,
    ):
        result = subprocess.run(
            command,
            cwd=runtime_dir,
            env=env,
            text=True,
            stdout=stdout,
            stderr=stderr,
            check=False,
        )
    executable_stat = executable.stat()
    metadata = {
        "command": command,
        "started_utc": started.isoformat(),
        "duration_seconds": time.monotonic() - monotonic_start,
        "executable_device": executable_stat.st_dev,
        "executable_inode": executable_stat.st_ino,
        "executable_sha256": sha256_file(executable),
        "criterion_home": str(criterion_home),
        "returncode": result.returncode,
        "runtime_directory": str(runtime_dir),
        "source_label": source_label,
    }
    (run_dir / "run.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n"
    )
    if result.returncode != 0:
        raise RunnerError(f"benchmark failed; inspect {run_dir}")


def collect_estimates(output_root: Path, rounds: int) -> list[dict[str, Any]]:
    measurements: list[dict[str, Any]] = []
    observed_runs: set[tuple[int, str]] = set()
    for run_dir in sorted((output_root / "runs").glob("[0-9][0-9][0-9]-[12]")):
        order = json.loads((run_dir / "order.json").read_text())
        round_number = int(order["round"])
        label = order["label"]
        observed_runs.add((round_number, label))
        criterion_home = run_dir / "criterion"
        for estimates_path in sorted(criterion_home.glob("**/new/estimates.json")):
            benchmark = estimates_path.relative_to(criterion_home).parts[:-2]
            estimates = json.loads(estimates_path.read_text())
            measurements.append(
                {
                    "round": round_number,
                    "label": label,
                    "benchmark": "/".join(benchmark),
                    "mean_ns": estimates["mean"]["point_estimate"],
                    "median_ns": estimates["median"]["point_estimate"],
                    "slope_ns": estimates.get("slope", {}).get("point_estimate", ""),
                }
            )
    expected_runs = {
        (round_number, label)
        for round_number in range(1, rounds + 1)
        for label in ("baseline", "candidate")
    }
    if observed_runs != expected_runs:
        raise RunnerError(
            f"run metadata mismatch: missing={sorted(expected_runs - observed_runs)}, "
            f"unexpected={sorted(observed_runs - expected_runs)}"
        )
    return measurements


def percentile(sorted_values: Sequence[float], probability: float) -> float:
    if not sorted_values:
        raise ValueError("cannot calculate a percentile of no values")
    position = (len(sorted_values) - 1) * probability
    lower = int(position)
    upper = min(lower + 1, len(sorted_values) - 1)
    fraction = position - lower
    return sorted_values[lower] * (1 - fraction) + sorted_values[upper] * fraction


def bootstrap_median_interval(
    values: Sequence[float], *, seed: int, resamples: int = 10_000
) -> tuple[float, float]:
    if not values:
        raise ValueError("cannot bootstrap no values")
    generator = random.Random(seed)
    medians = sorted(
        statistics.median(generator.choices(values, k=len(values)))
        for _ in range(resamples)
    )
    return percentile(medians, 0.025), percentile(medians, 0.975)


def summarize_measurements(
    measurements: Sequence[dict[str, Any]], seed: int
) -> list[dict[str, Any]]:
    indexed = {
        (row["benchmark"], int(row["round"]), row["label"]): float(row["mean_ns"])
        for row in measurements
    }
    benchmarks = sorted({row["benchmark"] for row in measurements})
    summaries: list[dict[str, Any]] = []
    for benchmark in benchmarks:
        rounds = sorted(
            {
                round_number
                for name, round_number, label in indexed
                if name == benchmark and label == "baseline"
            }
        )
        pairs = [
            (
                indexed[(benchmark, round_number, "baseline")],
                indexed[(benchmark, round_number, "candidate")],
            )
            for round_number in rounds
            if (benchmark, round_number, "candidate") in indexed
        ]
        if len(pairs) != len(rounds):
            raise RunnerError(f"missing candidate measurement for {benchmark}")
        ratios = [candidate / baseline for baseline, candidate in pairs]
        deltas = [(ratio - 1.0) * 100.0 for ratio in ratios]
        stable_seed = seed ^ int.from_bytes(
            hashlib.sha256(benchmark.encode()).digest()[:8], "big"
        )
        interval = bootstrap_median_interval(deltas, seed=stable_seed)
        summaries.append(
            {
                "benchmark": benchmark,
                "pairs": len(pairs),
                "baseline_median_mean_ns": statistics.median(pair[0] for pair in pairs),
                "candidate_median_mean_ns": statistics.median(
                    pair[1] for pair in pairs
                ),
                "median_delta_percent": statistics.median(deltas),
                "min_delta_percent": min(deltas),
                "max_delta_percent": max(deltas),
                "bootstrap_median_delta_low_percent": interval[0],
                "bootstrap_median_delta_high_percent": interval[1],
            }
        )
    return summaries


def write_csv(path: Path, rows: Sequence[dict[str, Any]]) -> None:
    if not rows:
        raise RunnerError(f"no rows available for {path.name}")
    with path.open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def write_status(path: Path, status: str, **details: Any) -> None:
    payload = {
        "status": status,
        "updated_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        **details,
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def environment_subset(environment: dict[str, str]) -> dict[str, str]:
    keys = (
        "RUSTFLAGS",
        "RUSTDOCFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_ENCODED_RUSTFLAGS",
        "CC",
        "CXX",
        "CFLAGS",
        "CXXFLAGS",
        "LDFLAGS",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "MALLOC_CONF",
    )
    selected = {key: environment[key] for key in keys if key in environment}
    selected.update(
        (key, value)
        for key, value in sorted(environment.items())
        if key.startswith("CARGO_PROFILE_")
    )
    return selected


def validate_args(args: argparse.Namespace) -> None:
    if args.rounds < 1:
        raise RunnerError("--rounds must be positive")
    if args.sample_size < 10:
        raise RunnerError("Criterion requires --sample-size >= 10")
    if args.warm_up_time <= 0 or args.measurement_time <= 0:
        raise RunnerError("warm-up and measurement times must be positive")
    if args.nresamples < 1:
        raise RunnerError("--nresamples must be positive")
    if args.cpu is not None and platform.system() != "Linux":
        raise RunnerError("--cpu requires Linux taskset; omit it on this platform")


def parse_args(arguments: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, help="baseline Git commit-ish")
    parser.add_argument("--candidate", required=True, help="candidate Git commit-ish")
    parser.add_argument("--filter", required=True, help="Criterion benchmark filter")
    parser.add_argument(
        "--exact", action="store_true", help="require an exact benchmark name"
    )
    parser.add_argument(
        "--bench", default="cpu", help="Cargo benchmark target (default: cpu)"
    )
    parser.add_argument(
        "--toolchain", default="1.86", help="rustup toolchain (default: 1.86)"
    )
    parser.add_argument(
        "--rounds", type=int, default=7, help="paired process rounds (default: 7)"
    )
    parser.add_argument(
        "--seed", type=int, default=0, help="recorded order/bootstrap seed"
    )
    parser.add_argument("--sample-size", type=int, default=100)
    parser.add_argument("--warm-up-time", type=float, default=3.0)
    parser.add_argument("--measurement-time", type=float, default=5.0)
    parser.add_argument("--nresamples", type=int, default=100_000)
    parser.add_argument(
        "--cpu", type=int, help="Linux CPU on which taskset pins each run"
    )
    parser.add_argument(
        "--clear-preload",
        action="store_true",
        help="remove LD_PRELOAD and DYLD_INSERT_LIBRARIES from build/run children",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="artifact directory (default: benchmark-results/<UTC timestamp>)",
    )
    parser.add_argument(
        "--keep-worktrees",
        action="store_true",
        help="retain temporary worktrees for debugging",
    )
    parser.add_argument(
        "--allow-harness-difference",
        action="store_true",
        help=(
            "allow Cargo.toml or benches to differ; record both digests in the "
            "manifest (use only after auditing the difference)"
        ),
    )
    args = parser.parse_args(arguments)
    validate_args(args)
    return args


def main(arguments: Sequence[str] | None = None) -> int:
    args = parse_args(arguments)
    repo = Path(command_output(["git", "rev-parse", "--show-toplevel"], Path.cwd()))
    if not repo.is_dir():
        raise RunnerError("run this command from inside a Git worktree")

    baseline_sha = resolve_commit(repo, args.baseline)
    candidate_sha = resolve_commit(repo, args.candidate)
    controlled_paths = ("Cargo.toml", "benches")
    baseline_digest = git_path_digest(repo, baseline_sha, controlled_paths)
    candidate_digest = git_path_digest(repo, candidate_sha, controlled_paths)
    harness_differs = baseline_digest != candidate_digest
    if harness_differs and not args.allow_harness_difference:
        raise RunnerError(
            "Cargo.toml or benches differ between commits; use a shared harness commit or "
            "external driver, or explicitly audit and allow the difference"
        )

    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output_root = (args.output or repo / "benchmark-results" / timestamp).resolve()
    if output_root.exists():
        raise RunnerError(f"output directory already exists: {output_root}")
    output_root.mkdir(parents=True)
    (output_root / "runs").mkdir()
    (output_root / "builds").mkdir()

    order = balanced_order(args.rounds, args.seed)
    effective_env = os.environ.copy()
    cleared_preloads = {}
    if args.clear_preload:
        for key in ("LD_PRELOAD", "DYLD_INSERT_LIBRARIES"):
            if key in effective_env:
                cleared_preloads[key] = effective_env.pop(key)
    metadata = {
        "schema_version": 2,
        "created_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "repository": str(repo),
        "baseline_ref": args.baseline,
        "baseline_commit": baseline_sha,
        "candidate_ref": args.candidate,
        "candidate_commit": candidate_sha,
        "controlled_paths": list(controlled_paths),
        "controlled_paths_match": not harness_differs,
        "baseline_controlled_paths_sha256": baseline_digest,
        "candidate_controlled_paths_sha256": candidate_digest,
        "allow_harness_difference": args.allow_harness_difference,
        "benchmark": args.bench,
        "filter": args.filter,
        "exact": args.exact,
        "rounds": args.rounds,
        "seed": args.seed,
        "order": order,
        "criterion": {
            "sample_size": args.sample_size,
            "warm_up_time_seconds": args.warm_up_time,
            "measurement_time_seconds": args.measurement_time,
            "nresamples": args.nresamples,
        },
        "cpu": args.cpu,
        "clear_preload": args.clear_preload,
        "toolchain": args.toolchain,
        "host": {
            "platform": platform.platform(),
            "python": sys.version,
            "uname": command_output(["uname", "-a"], repo),
            "lscpu": command_output(["lscpu"], repo),
            "rustc": command_output(["rustc", f"+{args.toolchain}", "-vV"], repo),
            "cargo": command_output(["cargo", f"+{args.toolchain}", "-V"], repo),
            "inherited_environment": environment_subset(dict(os.environ)),
            "effective_environment": environment_subset(effective_env),
            "cleared_preloads": cleared_preloads,
        },
    }
    if platform.system() == "Linux":
        governor = Path(
            f"/sys/devices/system/cpu/cpu{args.cpu or 0}/cpufreq/scaling_governor"
        )
        metadata["host"]["governor"] = (
            governor.read_text().strip() if governor.exists() else "unknown"
        )
    (output_root / "manifest.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n"
    )
    status_path = output_root / "status.json"
    write_status(status_path, "running")

    temporary_root = Path(tempfile.mkdtemp(prefix="thin-vec-bench-ab-"))
    worktrees = {
        "baseline": temporary_root / "baseline",
        "candidate": temporary_root / "candidate",
    }
    commits = {"baseline": baseline_sha, "candidate": candidate_sha}
    added_worktrees: list[Path] = []
    try:
        for label in ("baseline", "candidate"):
            run_checked(
                [
                    "git",
                    "worktree",
                    "add",
                    "--detach",
                    str(worktrees[label]),
                    commits[label],
                ],
                cwd=repo,
            )
            added_worktrees.append(worktrees[label])

        lock_command = cargo_command(args.toolchain, "generate-lockfile")
        lock_result = run_checked(
            lock_command, cwd=worktrees["baseline"], env=effective_env
        )
        (output_root / "builds" / "lock.stdout.log").write_text(lock_result.stdout)
        (output_root / "builds" / "lock.stderr.log").write_text(lock_result.stderr)
        shutil.copy2(
            worktrees["baseline"] / "Cargo.lock", worktrees["candidate"] / "Cargo.lock"
        )
        shutil.copy2(worktrees["baseline"] / "Cargo.lock", output_root / "Cargo.lock")

        executables: dict[str, Path] = {}
        binary_metadata_by_label: dict[str, dict[str, Any]] = {}
        for label in ("baseline", "candidate"):
            build_log = output_root / "builds" / label
            build_log.mkdir()
            executables[label] = build_benchmark(
                worktrees[label],
                temporary_root / f"target-{label}",
                args.toolchain,
                args.bench,
                "/thin-vec",
                build_log,
                effective_env,
            )
            binary_metadata = {
                "path": str(executables[label]),
                "size_bytes": executables[label].stat().st_size,
                "sha256": sha256_file(executables[label]),
            }
            retained_executable = build_log / f"{args.bench}-executable"
            shutil.copy2(executables[label], retained_executable)
            binary_metadata["artifact_path"] = str(
                retained_executable.relative_to(output_root)
            )
            if sha256_file(retained_executable) != binary_metadata["sha256"]:
                raise RunnerError(
                    f"retained {label} executable does not match its build"
                )
            binary_metadata_by_label[label] = binary_metadata
            (build_log / "binary.json").write_text(
                json.dumps(binary_metadata, indent=2, sort_keys=True) + "\n"
            )
        if (
            baseline_sha == candidate_sha
            and binary_metadata_by_label["baseline"]["sha256"]
            != binary_metadata_by_label["candidate"]["sha256"]
        ):
            raise RunnerError(
                "same-commit A/A builds produced different benchmark executables"
            )

        staged_executable = temporary_root / f"staged-{args.bench}"
        runtime_dir = temporary_root / "runtime"
        runtime_dir.mkdir()
        for round_number, labels in enumerate(order, start=1):
            for position, label in enumerate(labels, start=1):
                # Child-visible paths encode round and position, never the label.
                # Both position suffixes have equal length, and label assignment
                # alternates across rounds.
                run_dir = output_root / "runs" / f"{round_number:03d}-{position}"
                run_dir.mkdir()
                (run_dir / "order.json").write_text(
                    json.dumps(
                        {"round": round_number, "position": position, "label": label},
                        indent=2,
                        sort_keys=True,
                    )
                    + "\n"
                )
                print(
                    f"round {round_number}/{args.rounds}, position {position}: {label}",
                    flush=True,
                )
                # Execute both implementations from the same pathname and inode.
                # Otherwise loader path, file placement, or page-cache identity can
                # become a stable label-specific effect.
                shutil.copy2(executables[label], staged_executable)
                if (
                    sha256_file(staged_executable)
                    != binary_metadata_by_label[label]["sha256"]
                ):
                    raise RunnerError(
                        f"staged {label} executable does not match its build"
                    )
                run_benchmark(
                    staged_executable,
                    runtime_dir,
                    run_dir / "criterion",
                    run_dir,
                    args,
                    effective_env,
                    label,
                )

        measurements = collect_estimates(output_root, args.rounds)
        summaries = summarize_measurements(measurements, args.seed)
        write_csv(output_root / "measurements.csv", measurements)
        write_csv(output_root / "summary.csv", summaries)
        write_status(status_path, "complete", measurements=len(measurements))
        print(f"artifacts: {output_root}")
        return 0
    except Exception as error:
        write_status(status_path, "failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        if args.keep_worktrees:
            print(f"temporary worktrees retained: {temporary_root}", file=sys.stderr)
        else:
            for worktree in reversed(added_worktrees):
                subprocess.run(
                    ["git", "worktree", "remove", "--force", str(worktree)],
                    cwd=repo,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    check=False,
                )
            shutil.rmtree(temporary_root, ignore_errors=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RunnerError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2)
