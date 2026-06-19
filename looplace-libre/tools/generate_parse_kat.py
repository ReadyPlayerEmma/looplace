# SPDX-License-Identifier: MIT
"""Dev-only oracle: regenerate the record-parsing KATs baked into records.rs.

Drives glucometerutils' parsing (`_parse_record` / `_parse_arresult`) as an
oracle. NOT part of the build.

Requires the vendored reference package (gitignored) and the `attrs` library used
by glucometerutils' `common`. Run in a throwaway venv to avoid touching your env:

    python3 -m venv /tmp/oracle_venv
    /tmp/oracle_venv/bin/pip install attrs
    /tmp/oracle_venv/bin/python looplace-libre/tools/generate_parse_kat.py

Output values appear as assertions in looplace-libre/src/records.rs.
"""

import pathlib
import sys
import types

REPO = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / ".reference_packages/glucometerutils"))
# Stub freestyle_hid so support.freestyle imports without `construct`/hardware.
sys.modules["freestyle_hid"] = types.ModuleType("freestyle_hid")

from glucometerutils.support import freestyle_libre as L  # noqa: E402


def mkrec(pairs, n=46):
    rec = ["0"] * n
    for i, v in pairs:
        rec[i] = str(v)
    return rec


def main() -> None:
    hist = ["12", "0", "6", "19", "26", "8", "34", "0", "0", "0", "0", "0", "0", "105", "0", "0"]
    pr = L._parse_record(hist, L._HISTORY_ENTRY_MAP)
    print("history:", pr, "ts:", L._extract_timestamp(pr).isoformat())

    cases = {
        "scan_food_long": mkrec([(0, 12), (1, 2), (2, 6), (3, 19), (4, 26), (5, 9), (6, 0), (7, 0), (9, 2), (12, 120), (18, 1), (23, 10), (25, 1), (26, 30), (28, 0)]),
        "blood": mkrec([(0, 12), (1, 2), (2, 6), (3, 19), (4, 26), (5, 9), (6, 5), (7, 0), (9, 0), (12, 98), (28, 0)]),
        "ketone_18": mkrec([(0, 12), (1, 2), (2, 6), (3, 19), (4, 26), (5, 9), (6, 10), (7, 0), (9, 1), (12, 18), (28, 0)]),
        "scan_rapid": mkrec([(0, 12), (1, 2), (2, 6), (3, 19), (4, 26), (5, 9), (6, 15), (7, 0), (9, 2), (12, 110), (17, 1), (43, 7), (28, 0)]),
        "time_adjust": mkrec([(0, 12), (1, 5), (2, 6), (3, 19), (4, 26), (5, 10), (6, 0), (7, 0), (9, 6), (10, 19), (11, 26), (12, 9), (13, 30), (14, 0)], n=15),
    }
    for name, rec in cases.items():
        print(f"{name}: {L._parse_arresult(rec)!r}")


if __name__ == "__main__":
    main()
