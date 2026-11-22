# Adversarial challenge kits

These directories are the adversarial review kits written by an outside
reviewer (ChatGPT) for this project and relayed by the project owner.
They are test programs and test tooling only; no compiler code comes from
them. They are kept byte-for-byte as received, so each kit's own
`SHA256SUMS.txt` still checks (`sha256sum -c SHA256SUMS.txt` inside the
kit). Two parts of each original zip are left out to keep the tree small:

- `reference-results/`: the reviewer's own x86-64 reference runs.
- `probes/`: standards probes that are not gating tests.

The matching `SHA256SUMS.txt` lines were removed for those two folders
only; every other line is unchanged.

| Kit | Cases | What it adds |
| --- | ---: | --- |
| `hcc-challenge-v0.7` | 65 | `_Alignas` records, VLAs, `_Atomic`, TLS, `<threads.h>`, K&R, `<tgmath.h>`, GNU |
| `hcc-challenge-v0.8` | 80 | layout and ABI pairs, atomics, threads, diagnostics |
| `hcc-challenge-v0.9` | 100 | 27 thread/TLS/TSS/once cases, memory ordering, concurrent programs, layout/ABI pairs; 33 cases are marked for repeated stress runs |

The kits' own runners (`runner.py`, `stress_threads.py`, and in v0.9
`mixed_reference.py`, `generated_campaign.py`, `cross_target.py` and
`ir_inventory.py`) run only on native Linux: the v0.7 and v0.8 runners
on x86-64 only, the v0.9 ones on x86-64 or AArch64.
`tools/platform/challenge_native.py` runs the same cases on any of those
hosts, on macOS and on Windows, and with `--sanitize
address|thread|undefined` under the sanitizers. The weekly real-platform workflow
(`.github/workflows/weekly-platforms.yml`, described in
`docs/WEEKLY_PLATFORMS.md`) uses all of them. The standard pull-request CI
does not run these kits.

The earlier v0.1 to v0.6 kits (724 cases) are larger (about 9 MB of cases)
and are not in the tree.
