#!/usr/bin/env python3
"""
Parse the 5-year playoff-skater export into a normalized CSV.

Source file: tab-separated export with a 3-line section/column header and
data rows that span 51 fields. Rows for players traded within a season
carry a literal `TOT \\n` in the team field which splits the row across
two visible lines. We join those back together before parsing.

Output columns (CSV, with header):
    player_name, team, position, born, gp, g, a, p, shots, toi_seconds

Enough for the Bayesian player-projection blend. Dropping the per-60 /
per-GP / situational breakdowns keeps the CSV compact; the raw totals are
all we need as a regression prior. Players with `TOT` team get their
multi-team aggregated row; per-team splits (which immediately follow TOT
in the source) are dropped.

Run:
    python3 backend/scripts/parse_historical_playoff_skaters.py \\
        /Users/bastian/Downloads/last_5_playoff_seasons_skaters.txt \\
        backend/data/historical_playoff_skater_totals.csv
"""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path

NORMAL_FIELDS = 51  # Complete data row field count.
TRADED_MARKER = "TOT"
EXPECTED_OUT_COLS = [
    "player_name",
    "team",
    "position",
    "born",
    "gp",
    "g",
    "a",
    "p",
    "shots",
    "toi_seconds",
]


def toi_to_seconds(toi: str) -> int | None:
    """Convert `MM:SS` TOI notation to seconds. Empty or malformed → None."""
    if not toi:
        return None
    m = re.match(r"^\s*(\d+):(\d{2})\s*$", toi)
    if not m:
        return None
    return int(m.group(1)) * 60 + int(m.group(2))


def load_rows(path: Path) -> list[list[str]]:
    """Return the raw data rows (after header), field-split on tab, with
    TOT-split rows rejoined. Rows below the header are 3..n."""
    text = path.read_text(encoding="utf-8")
    raw_lines = text.split("\n")
    # Drop trailing blank lines.
    while raw_lines and not raw_lines[-1].strip():
        raw_lines.pop()

    # Lines 0..2 are header (section row, then two half-headers). Data starts at 3.
    data_lines = raw_lines[3:]
    rows: list[list[str]] = []
    i = 0
    while i < len(data_lines):
        fields = data_lines[i].split("\t")
        # TOT-split rows end with "TOT " in the team column (field idx 3) and
        # have ~4 fields before a literal newline. Detect and splice.
        if len(fields) < NORMAL_FIELDS and i + 1 < len(data_lines):
            next_fields = data_lines[i + 1].split("\t")
            # Only splice if the team field looks like "TOT " / "TOT" — avoids
            # accidentally swallowing trailing blanks.
            team_field = fields[3] if len(fields) > 3 else ""
            if team_field.strip() == TRADED_MARKER:
                # Strip trailing whitespace from the partial team field, then
                # concatenate. Lose the leading empty string from the next
                # line (byproduct of the `\n\t` split).
                merged = fields[:3] + [TRADED_MARKER] + next_fields[1:]
                if len(merged) == NORMAL_FIELDS:
                    rows.append(merged)
                    i += 2
                    continue
        if len(fields) == NORMAL_FIELDS:
            rows.append(fields)
        # Silently drop malformed rows — they're rare (<5 in the source) and
        # not worth the complexity of a recovery heuristic.
        i += 1
    return rows


def project(row: list[str]) -> dict[str, object] | None:
    """Pick the columns we care about out of a 51-field row."""
    try:
        # Field order (1-indexed per source header):
        #   1 Rk  2 Nat  3 Name  4 Team  5 Born  6 Pos  7 GP  8 G  9 A  10 P
        #   11 PIM  12 +/-  13 TOI  ...  45 SOG  46 SH%  47 HITS  ...
        player_name = row[2].strip()
        team = row[3].strip()
        position = row[5].strip()
        born = int(row[4]) if row[4].strip().isdigit() else None
        gp = int(row[6]) if row[6].strip().isdigit() else None
        g = int(row[7]) if row[7].strip().isdigit() else 0
        a = int(row[8]) if row[8].strip().isdigit() else 0
        p = int(row[9]) if row[9].strip().isdigit() else 0
        toi = toi_to_seconds(row[12])
        shots_str = row[44].strip() if len(row) > 44 else ""
        shots = int(shots_str) if shots_str.isdigit() else None

        if not player_name or not team or gp is None:
            return None
        return {
            "player_name": player_name,
            "team": team,
            "position": position,
            "born": born,
            "gp": gp,
            "g": g,
            "a": a,
            "p": p,
            "shots": shots,
            "toi_seconds": toi,
        }
    except (IndexError, ValueError):
        return None


def main(src: str, dst: str) -> int:
    src_path = Path(src)
    dst_path = Path(dst)
    if not src_path.exists():
        print(f"source not found: {src}", file=sys.stderr)
        return 1

    rows = load_rows(src_path)
    out_rows: list[dict[str, object]] = []
    for r in rows:
        projected = project(r)
        if projected is not None:
            out_rows.append(projected)

    dst_path.parent.mkdir(parents=True, exist_ok=True)
    with dst_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=EXPECTED_OUT_COLS)
        writer.writeheader()
        for row in out_rows:
            writer.writerow(row)

    print(f"wrote {len(out_rows)} rows to {dst}")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(
            "usage: parse_historical_playoff_skaters.py <input.txt> <output.csv>",
            file=sys.stderr,
        )
        sys.exit(2)
    sys.exit(main(sys.argv[1], sys.argv[2]))
