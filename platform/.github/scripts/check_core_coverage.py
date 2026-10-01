#!/usr/bin/env python3
"""Fail CI when coverage of the correctness-critical ledger modules drops
below the bar.

Reads an lcov.info produced by `cargo llvm-cov --lcov`. The PixelCraft
standard gives the ledger/transaction/balance code path the highest bar in
the repo (85%+), rather than chasing an overall average — money math and the
posting engine are where a bug costs real money.

Usage: check_core_coverage.py [lcov.info]
"""

import sys
from pathlib import Path

# The correctness-critical modules. These carry the ledger invariants:
# engine.rs (posting, holds, idempotency), balances.rs (derived balances),
# money.rs (fee math), reconcile.rs (rail matching).
CORE_FILES = [
    "ledger/src/engine.rs",
    "ledger/src/balances.rs",
    "ledger/src/money.rs",
    "ledger/src/reconcile.rs",
]
THRESHOLD = 0.85  # PixelCraft target: 85-90% on ledger/transaction/balance logic


def parse_lcov(path: Path) -> dict[str, tuple[int, int]]:
    """SF:/LF:/LH: sections -> {path: (lines_hit, lines_found)}."""
    files: dict[str, tuple[int, int]] = {}
    current: str | None = None
    found = 0
    for line in path.read_text().splitlines():
        if line.startswith("SF:"):
            current = line[3:].replace("\\", "/")
        elif line.startswith("LF:") and current is not None:
            found = int(line[3:])
        elif line.startswith("LH:") and current is not None:
            files[current] = (int(line[3:]), found)
        elif line == "end_of_record":
            current = None
    return files


def main() -> int:
    lcov_path = Path(sys.argv[1] if len(sys.argv) > 1 else "lcov.info")
    if not lcov_path.exists():
        print(f"coverage report not found: {lcov_path}", file=sys.stderr)
        return 1
    files = parse_lcov(lcov_path)

    failures: list[str] = []
    for rel in CORE_FILES:
        # Match on the normalized suffix so absolute/relative paths both work.
        key = next((k for k in files if k.endswith("/" + rel)), None)
        if key is None:
            print(f"[MISSING] {rel}: not present in coverage report", file=sys.stderr)
            failures.append(rel)
            continue
        hit, found = files[key]
        pct = hit / found if found else 0.0
        ok = pct >= THRESHOLD
        print(f"[{'ok' if ok else 'FAIL'}] {rel}: {hit}/{found} lines ({pct:.1%}, bar {THRESHOLD:.0%})")
        if not ok:
            failures.append(rel)

    if failures:
        print(
            f"coverage below {THRESHOLD:.0%} for: {', '.join(failures)}",
            file=sys.stderr,
        )
        return 1
    print(f"all core modules >= {THRESHOLD:.0%} coverage")
    return 0


if __name__ == "__main__":
    sys.exit(main())