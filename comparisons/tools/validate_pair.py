#!/usr/bin/env python3
"""Reject physical reports that are not scientifically comparable."""

import argparse
import json
from pathlib import Path

from run_matrix import pair_issues


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=Path)
    parser.add_argument("right", type=Path)
    args = parser.parse_args()
    left = json.loads(args.left.read_text())
    right = json.loads(args.right.read_text())
    issues = pair_issues(left, right)
    if issues:
        raise SystemExit("reports are not comparable:\n- " + "\n- ".join(issues))
    print(
        f"comparable: {left['platform_id']} and {right['platform_id']} "
        f"at {left['metadata']['git_commit']} using {left['toolchain']}"
    )


if __name__ == "__main__":
    main()
