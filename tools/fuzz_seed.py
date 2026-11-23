#!/usr/bin/env python3
"""Seed the cargo-fuzz corpora from this repository's own C tests.

Every `.c` and `.h` file under tests/ (external suites excluded, since
they are fetched, not part of the repository) is copied, by content hash,
into fuzz/corpus/<target>/ for each fuzz target. Files over 64 KiB are
skipped, matching the size limit in fuzz/fuzz_targets/common.rs.

Usage: python3 tools/fuzz_seed.py [target ...]
"""
import hashlib
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TARGETS = ["lex", "preprocess", "parse", "frontend"]
LIMIT = 64 * 1024


def seeds():
    for path in sorted((ROOT / "tests").rglob("*")):
        if path.suffix not in (".c", ".h") or not path.is_file():
            continue
        if "external" in path.relative_to(ROOT / "tests").parts:
            continue
        data = path.read_bytes()
        if len(data) <= LIMIT:
            yield data


def main():
    targets = sys.argv[1:] or TARGETS
    data = list(seeds())
    for target in targets:
        out = ROOT / "fuzz" / "corpus" / target
        out.mkdir(parents=True, exist_ok=True)
        for blob in data:
            (out / hashlib.sha1(blob).hexdigest()).write_bytes(blob)
        print(f"{target}: {len(data)} seeds in {out.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
