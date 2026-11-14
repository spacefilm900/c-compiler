#!/usr/bin/env python3
"""Write reports/perf/scorecard.{md,json} from baseline.json and after.json.

Both inputs come from tools/bench_compile.py. The markdown has the summary
tables; the notes section is kept from an existing scorecard.md (everything
from the line '## Notes' on).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

PERF = Path(__file__).resolve().parent.parent / "reports" / "perf"
PHASES = ["lex", "preprocess", "parse", "sema", "irgen", "write"]


def rev(data: dict) -> str:
    """The measured compiler's revision; older files only record `commit`."""
    return data.get("compiler_rev", data["commit"])


def check_units(before: dict, after: dict) -> list[str]:
    """Differences between the two runs' groups and files, as messages."""
    problems = []
    for name in sorted(set(before["groups"]) | set(after["groups"])):
        if name not in before["groups"] or name not in after["groups"]:
            which = "baseline" if name not in before["groups"] else "after"
            problems.append(f"input {name} is missing from the {which} run")
            continue
        b = set(before["groups"][name]["units"])
        a = set(after["groups"][name]["units"])
        if b != a:
            problems.append(
                f"input {name} covers different files: only in baseline {sorted(b - a)}, "
                f"only in after {sorted(a - b)}"
            )
    if before["opts"] != after["opts"]:
        problems.append(f"optimisation levels differ: {before['opts']} vs {after['opts']}")
    return problems


def main() -> int:
    before = json.loads((PERF / "baseline.json").read_text())
    after = json.loads((PERF / "after.json").read_text())
    problems = check_units(before, after)
    if problems:
        for problem in problems:
            print(f"perf_scorecard: {problem}", file=sys.stderr)
        print("perf_scorecard: the runs are not comparable; no scorecard written", file=sys.stderr)
        return 1
    opts = before["opts"]
    rows = {}
    for name, g in before["groups"].items():
        b, a = g["total"], after["groups"][name]["total"]
        row = {
            "tus": len(g["units"]),
            "frontend_s": [b["front_wall"], a["front_wall"]],
            "frontend_peak_mib": [b["front_rss"], a["front_rss"]],
            "phases_s": {p: [b.get(f"phase_{p}", 0.0), a.get(f"phase_{p}", 0.0)] for p in PHASES},
        }
        for o in opts:
            row[o] = {
                "backend_s": [b[f"{o}_back_wall"], a[f"{o}_back_wall"]],
                "ours_s": [b[f"{o}_ours_wall"], a[f"{o}_ours_wall"]],
                "clang_s": [b[f"{o}_clang_wall"], a[f"{o}_clang_wall"]],
                "ratio": [b[f"{o}_ratio"], a[f"{o}_ratio"]],
                "clang_peak_mib": a[f"{o}_clang_rss"],
            }
        rows[name] = row
    data = {
        "before_commit": rev(before),
        "after_commit": rev(after),
        "clang": after["clang"],
        "machine": after["machine"],
        "runs": after["runs"],
        "opts": opts,
        "inputs": rows,
    }
    (PERF / "scorecard.json").write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")

    md = PERF / "scorecard.md"
    notes = ""
    if md.exists():
        text = md.read_text()
        if "## Notes" in text:
            notes = text[text.index("## Notes") :]
    out = [
        "# Compile-time performance scorecard",
        "",
        f"- Before: `{rev(before)}` ([baseline.md](baseline.md), frozen); after: `{rev(after)}` ([after.md](after.md))",
        f"- Reference: {after['clang']}",
        f"- Machine: {after['machine']}",
        f"- Median of {after['runs']} runs per TU; group rows sum the TUs; peak memory is the largest TU",
        "",
        "Ours = this compiler's frontend plus `clang -O<n> -c` on its IR. Clang = Clang compiling the C directly.",
        "",
        "## Totals, before -> after",
        "",
        "| Input | TUs | Frontend | Frontend peak | " + " | ".join(f"{o} ours | {o} Clang | {o} ratio" for o in opts) + " |",
        "|---|---|---|---|" + "---|---|---|" * len(opts),
    ]
    for name, r in rows.items():
        f0, f1 = r["frontend_s"]
        m0, m1 = r["frontend_peak_mib"]
        cells = [name, str(r["tus"]), f"{f0:.2f} s -> {f1:.2f} s ({f0 / f1:.0f}x)", f"{m0:.0f} -> {m1:.0f} MiB"]
        for o in opts:
            x = r[o]
            cells += [
                f"{x['ours_s'][0]:.2f} s -> {x['ours_s'][1]:.2f} s",
                f"{x['clang_s'][1]:.2f} s",
                f"{x['ratio'][0]:.2f}x -> {x['ratio'][1]:.2f}x",
            ]
        out.append("| " + " | ".join(cells) + " |")
    out += ["", "## Frontend phases, before -> after (seconds)", ""]
    out.append("| Input | " + " | ".join(PHASES) + " |")
    out.append("|---|" + "---|" * len(PHASES))
    for name, r in rows.items():
        out.append(
            f"| {name} | " + " | ".join(f"{r['phases_s'][p][0]:.2f} -> {r['phases_s'][p][1]:.2f}" for p in PHASES) + " |"
        )
    out += ["", "## LLVM backend on our IR vs Clang's whole compile (after)", ""]
    out.append("| Input | " + " | ".join(f"{o} backend | {o} Clang" for o in opts) + " |")
    out.append("|---|" + "---|---|" * len(opts))
    for name, r in rows.items():
        out.append(
            f"| {name} | " + " | ".join(f"{r[o]['backend_s'][1]:.2f} s | {r[o]['clang_s'][1]:.2f} s" for o in opts) + " |"
        )
    out.append("")
    md.write_text("\n".join(out) + ("\n" + notes if notes else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
