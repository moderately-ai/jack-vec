#!/usr/bin/env python3
"""Generate deterministic, reviewable benchmark graphics from authoritative reports."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import matplotlib as mpl
import matplotlib.pyplot as plt
import numpy as np


IMPLEMENTATIONS = ["Vec", "JackVec", "ThinVec", "SmallVec4", "SmallVec8"]
COLORS = {
    "Vec": "#64748b",
    "JackVec": "#e11d48",
    "ThinVec": "#7c3aed",
    "SmallVec4": "#0284c7",
    "SmallVec8": "#059669",
}


def save_svg(fig, output: Path) -> None:
    fig.savefig(output, metadata={"Date": None})
    # Matplotlib writes trailing spaces in multiline path data. Normalize its
    # otherwise deterministic output so repository whitespace checks stay useful.
    output.write_text("\n".join(line.rstrip() for line in output.read_text().splitlines()) + "\n")
    plt.close(fig)


def load_report(path: Path) -> dict:
    report = json.loads(path.read_text())
    if report.get("schema_version") != 1:
        raise ValueError(f"{path}: unsupported schema version")
    if report.get("metadata", {}).get("authoritative") is not True:
        raise ValueError(f"{path}: report is not authoritative")
    return report


def matrix(rows: list[dict], key, value) -> tuple[list[str], np.ndarray]:
    labels = sorted({key(row) for row in rows})
    lookup = {(key(row), row["implementation"]): value(row) for row in rows}
    values = np.array([[lookup[(label, impl)] for impl in IMPLEMENTATIONS] for label in labels])
    return labels, values


def heatmap(labels, values, title, subtitle, output: Path) -> None:
    height = max(5.0, 0.34 * len(labels) + 2.0)
    fig, ax = plt.subplots(figsize=(10.5, height), layout="constrained")
    # A discrete palette keeps both the cells and colorbar as portable SVG paths;
    # Matplotlib rasterizes a continuous colorbar using platform-specific PNG bytes.
    palette = mpl.colormaps["RdYlGn_r"].resampled(9)
    image = ax.pcolormesh(np.log2(np.clip(values, 0.5, 2.0)), cmap=palette,
                         vmin=-1, vmax=1, shading="flat")
    ax.set_xlim(0, len(IMPLEMENTATIONS))
    ax.set_ylim(len(labels), 0)
    ax.set_xticks(np.arange(len(IMPLEMENTATIONS)) + 0.5, IMPLEMENTATIONS, fontweight="bold")
    ax.set_yticks(np.arange(len(labels)) + 0.5, labels, fontsize=8)
    ax.tick_params(length=0)
    for row in range(values.shape[0]):
        for col in range(values.shape[1]):
            ratio = values[row, col]
            ax.text(col + 0.5, row + 0.5, f"{ratio:.2f}×", ha="center", va="center", fontsize=7,
                    color="white" if ratio < 0.58 or ratio > 1.72 else "#111827")
    ax.set_title(title, loc="left", fontsize=16, fontweight="bold", pad=24)
    ax.text(0, 1.015, subtitle, transform=ax.transAxes, fontsize=9, color="#475569")
    colorbar = fig.colorbar(image, ax=ax, shrink=0.5, pad=0.02)
    colorbar.set_ticks([-1, 0, 1], labels=["0.5×", "1×", "2×"])
    colorbar.set_label("ratio to Vec (lower is better)")
    save_svg(fig, output)


def performance_profile(cpu: list[dict], platform: str, output: Path) -> None:
    labels, times = matrix(cpu, lambda row: row["benchmark"], lambda row: row["median_ns"])
    del labels
    relative = times / times.min(axis=1, keepdims=True)
    factors = np.linspace(1.0, 2.0, 201)
    fig, ax = plt.subplots(figsize=(10.5, 5.8), layout="constrained")
    for col, impl in enumerate(IMPLEMENTATIONS):
        fractions = [(relative[:, col] <= factor).mean() for factor in factors]
        ax.plot(factors, fractions, label=impl, color=COLORS[impl], linewidth=2.4)
    ax.set(xlim=(1, 2), ylim=(0, 1.01), xlabel="factor of fastest implementation (lower is better)",
           ylabel="share of CPU workloads at or below factor")
    ax.grid(color="#e2e8f0", linewidth=0.8)
    ax.set_title("CPU performance profile", loc="left", fontsize=16, fontweight="bold", pad=24)
    ax.text(0, 1.015, f"{platform} · every workload retained · higher and further left is better",
            transform=ax.transAxes, fontsize=9, color="#475569")
    ax.legend(ncol=5, frameon=False, loc="lower right")
    save_svg(fig, output)


def allocation_label(row: dict) -> str:
    parts = [row["benchmark"], row["input"], f"element={row['element_size']}B"]
    return "/".join(parts)


def comparison_counts(cpu: list[dict], left: str, right: str) -> tuple[int, int, int]:
    by_benchmark: dict[str, dict[str, float]] = {}
    for row in cpu:
        by_benchmark.setdefault(row["benchmark"], {})[row["implementation"]] = row["median_ns"]
    ratios = [rows[left] / rows[right] for rows in by_benchmark.values()]
    return (sum(ratio < 0.97 for ratio in ratios),
            sum(0.97 <= ratio <= 1.03 for ratio in ratios),
            sum(ratio > 1.03 for ratio in ratios))


def write_latest(report: dict, output: Path) -> None:
    metadata = report["metadata"]
    audits = metadata["runtime_audits"]
    classifications = {
        impl: {name: 0 for name in ("win", "equivalent", "inconclusive", "loss")}
        for impl in IMPLEMENTATIONS[1:]
    }
    for row in report["cpu"]:
        if row["implementation"] in classifications:
            classifications[row["implementation"]][row["classification"]] += 1
    classification_rows = "\n".join(
        f"| {impl} | {counts['win']} | {counts['equivalent']} | {counts['inconclusive']} | {counts['loss']} |"
        for impl, counts in classifications.items()
    )
    head_rows = []
    for other in ("Vec", "ThinVec", "SmallVec4", "SmallVec8"):
        faster, close, slower = comparison_counts(report["cpu"], "JackVec", other)
        head_rows.append(f"| {other} | {faster} | {close} | {slower} |")
    head_to_head_rows = "\n".join(head_rows)
    cpu = {(row["benchmark"], row["implementation"]): row for row in report["cpu"]}
    alloc = {
        (row["benchmark"], row["input"], row["element_size"], row["implementation"]): row
        for row in report["allocations"]
    }
    def cpu_ratio(benchmark: str, left: str, right: str) -> float:
        return cpu[benchmark, left]["median_ns"] / cpu[benchmark, right]["median_ns"]

    def requested_ratio(benchmark: str, input_: str, element_size: str, left: str, right: str) -> float:
        return (float(alloc[benchmark, input_, element_size, left]["live_requested"])
                / float(alloc[benchmark, input_, element_size, right]["live_requested"]))

    largest_jack_gaps = sorted(
        ((row["median_ns"] / cpu[row["benchmark"], "Vec"]["median_ns"], row["benchmark"])
         for row in report["cpu"] if row["implementation"] == "JackVec"),
        reverse=True,
    )[:3]
    gap_summary = ", ".join(f"`{benchmark}` ({ratio:.3f}×)" for ratio, benchmark in largest_jack_gaps)

    output.write_text(f"""# Latest benchmark comparison

This is the authoritative `{report['platform_id']}` baseline. Lower ratios are
better. CPU classifications and heatmap ratios use `Vec` as the baseline; red
does not mean an implementation lost to every other candidate. Every measured
implementation and scenario is retained, and platforms are never pooled.

![CPU performance profile](graphics/{report['platform_id']}-cpu-profile.svg)

![Complete CPU ratio heatmap](graphics/{report['platform_id']}-cpu-heatmap.svg)

## What this baseline says

- JackVec is not an across-the-board faster `Vec`: it has
  {classifications['JackVec']['win']} confidence-qualified wins and
  {classifications['JackVec']['loss']} losses versus `Vec` in this matrix.
- Its intended nested-density advantage is substantial: requested memory for the
  empty and sparse nested workloads is
  {requested_ratio('nested', 'empty', '8', 'JackVec', 'Vec'):.3f}× and
  {requested_ratio('nested', 'sparse', '8', 'JackVec', 'Vec'):.3f}× Vec,
  respectively, while each collection owner remains one machine word.
- The optimized large append path reaches
  {cpu_ratio('append_preallocated/1024', 'JackVec', 'Vec'):.3f}× Vec and
  {cpu_ratio('append_preallocated/1024', 'JackVec', 'ThinVec'):.3f}× upstream
  ThinVec. This is a large targeted improvement, not a universal CPU claim.
- JackVec's three largest median CPU gaps versus Vec are {gap_summary}. They are
  retained here as investigation targets; confidence-aware classifications remain
  authoritative over point-estimate ordering.
- Against the inline candidates, JackVec wins most measured CPU medians, while
  SmallVec avoids heap allocation when values fit inline. Neither representation
  dominates every workload.

## CPU outcomes

The confidence-aware classifications below compare each implementation with
`Vec`. “Inconclusive” means the paired 95% interval crosses a boundary; it is not
silently counted as equality.

| Implementation | Wins | Equivalent | Inconclusive | Losses |
|---|---:|---:|---:|---:|
{classification_rows}

For direct context, this simpler head-to-head table compares median CPU times
using the same ±3% practical band. It does not replace the confidence-aware table.

| JackVec compared with | JackVec faster | Within ±3% | JackVec slower |
|---|---:|---:|---:|
{head_to_head_rows}

## Memory outcomes

Requested and allocator-usable heap are deliberately separate. Requested bytes
show representation savings; usable bytes show what the measured allocator
actually retained after size-class rounding.

![Requested live heap ratio heatmap](graphics/{report['platform_id']}-memory-requested-heatmap.svg)

![Allocator-usable live heap ratio heatmap](graphics/{report['platform_id']}-memory-usable-heatmap.svg)

Collection-owner size is not included in those heap ratios. A `Vec` owner is 24
bytes, a JackVec or ThinVec owner is 8 bytes, and SmallVec owners vary with inline
capacity and element alignment. In nested rows the outer allocation already
contains every inner owner, so adding the owner column to live heap would double
count memory. See [the complete platform table]({report['platform_id']}.md) for
owner bytes, absolute requested/usable bytes, allocation counts, reallocations,
and spill counts.

## Run provenance

- Commit: `{metadata['git_commit']}`
- Compiler: `{metadata['compiler_identity']['release']}` (`{metadata['compiler_identity']['commit_hash']}`)
- Allocator policy: `{metadata['allocator']['policy']}`; inherited injection:
  `{metadata['allocator']['inherited_environment']}`; effective injection:
  `{metadata['allocator']['effective_environment']}`
- CPU rounds: {report['round_count']}; CPU rows: {len(report['cpu'])}; allocation rows: {len(report['allocations'])}
- Minimum pinned-core idle audit: {min(a['cpu_idle_percent'] for a in audits):.1f}%
- Maximum audited one-minute load: {max(a['load_average'][0] for a in audits):.2f}

The performance profile reports the fraction of workloads within each factor of
the fastest implementation for that workload. It is an aggregate view, not a
claim that all workloads are equally representative. The heatmaps preserve the
individual results. These microbenchmarks describe the listed operations, element
types, sizes, compiler, allocator, and machine—not every application. macOS
remains pending until a clean authoritative run is available.
""")


def generate(report_path: Path, results_dir: Path) -> None:
    report = load_report(report_path)
    platform = report["platform_id"]
    graphics = results_dir / "graphics"
    graphics.mkdir(parents=True, exist_ok=True)
    cpu_labels, cpu_values = matrix(report["cpu"], lambda row: row["benchmark"], lambda row: row["ratio_to_vec"])
    heatmap(cpu_labels, cpu_values, "CPU time by workload", f"{platform} · paired medians relative to Vec",
            graphics / f"{platform}-cpu-heatmap.svg")
    alloc_labels, requested = matrix(report["allocations"], allocation_label,
                                     lambda row: float(row["live_requested"]))
    requested /= requested[:, :1]
    heatmap(alloc_labels, requested, "Requested live heap by workload",
            f"{platform} · requested bytes relative to Vec · zero means inline storage",
            graphics / f"{platform}-memory-requested-heatmap.svg")
    _, usable = matrix(report["allocations"], allocation_label,
                       lambda row: float(row["live_usable"]))
    usable /= usable[:, :1]
    heatmap(alloc_labels, usable, "Allocator-usable live heap by workload",
            f"{platform} · usable bytes relative to Vec · includes allocator size-class rounding",
            graphics / f"{platform}-memory-usable-heatmap.svg")
    performance_profile(report["cpu"], platform, graphics / f"{platform}-cpu-profile.svg")
    write_latest(report, results_dir / "LATEST.md")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    parser.add_argument("--results-dir", type=Path, default=Path("comparisons/benchmark-results"))
    args = parser.parse_args()
    mpl.rcParams.update({"font.family": "DejaVu Sans", "svg.hashsalt": "jack-vec"})
    generate(args.report, args.results_dir)


if __name__ == "__main__":
    main()
