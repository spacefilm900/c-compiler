# Compile-time benchmark: baseline (before the performance phase)

- Date: 2026-11-14
- Compiler commit: `d57f313`
- Reference: Debian clang version 19.1.7 (3+b1)
- Machine: x86_64, 8 CPUs, Linux-6.12.113+-x86_64-with-glibc2.41
- Runs per TU: 3 (median kept)

Ours = this compiler's frontend plus `clang -O<n> -c` on its IR. Clang = Clang compiling the C
directly at the same level. Group rows are sums of per-TU medians; peak memory is the largest TU.

| Input | TUs | Frontend | O0 backend | O0 ours | O0 Clang | O0 ratio | O2 backend | O2 ours | O2 Clang | O2 ratio | Front peak | Clang peak | IR size |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sqlite3 | 1 | 111.35 s | 1.02 s | 112.37 s | 1.24 s | 90.40x | 18.63 s | 129.99 s | 20.12 s | 6.46x | 698.2 MiB | 271.8 MiB | 15.1 MB |
| shell | 1 | 5.59 s | 0.26 s | 5.84 s | 0.32 s | 18.24x | 3.04 s | 8.63 s | 3.13 s | 2.76x | 240.7 MiB | 139.4 MiB | 3.7 MB |
| lua | 34 | 4.45 s | 0.92 s | 5.36 s | 1.23 s | 4.36x | 3.57 s | 8.01 s | 3.73 s | 2.15x | 112.6 MiB | 116.4 MiB | 3.9 MB |
| doom | 82 | 7.33 s | 1.82 s | 9.15 s | 2.36 s | 3.87x | 4.49 s | 11.82 s | 5.27 s | 2.24x | 68.3 MiB | 109.2 MiB | 5.5 MB |
| torture | 5 | 0.10 s | 0.14 s | 0.24 s | 0.15 s | 1.60x | 0.50 s | 0.60 s | 0.52 s | 1.16x | 16.1 MiB | 106.0 MiB | 0.9 MB |

## Frontend phases (seconds)

| Input | lex | preprocess | parse | sema | irgen | write |
|---|---|---|---|---|---|---|
| sqlite3 | 1.26 | 108.06 | 0.57 | 0.71 | 0.24 | 0.03 |
| shell | 0.41 | 4.84 | 0.09 | 0.11 | 0.05 | 0.01 |
| lua | 1.88 | 2.05 | 0.22 | 0.10 | 0.05 | 0.01 |
| doom | 3.41 | 2.91 | 0.40 | 0.25 | 0.08 | 0.01 |
| torture | 0.03 | 0.04 | 0.01 | 0.01 | 0.01 | 0.00 |
