# Compile-time benchmark: after the performance phase

- Date: 2026-11-14
- Compiler commit: `44cc353`
- Reference: Debian clang version 19.1.7 (3+b1)
- Machine: x86_64, 8 CPUs, Linux-6.12.113+-x86_64-with-glibc2.41
- Runs per TU: 3 (median kept)

Ours = this compiler's frontend plus `clang -O<n> -c` on its IR. Clang = Clang compiling the C
directly at the same level. Group rows are sums of per-TU medians; peak memory is the largest TU.

| Input | TUs | Frontend | O0 backend | O0 ours | O0 Clang | O0 ratio | O2 backend | O2 ours | O2 Clang | O2 ratio | Front peak | Clang peak | IR size |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sqlite3 | 1 | 1.34 s | 1.01 s | 2.34 s | 1.33 s | 1.77x | 19.17 s | 20.51 s | 19.87 s | 1.03x | 320.4 MiB | 271.5 MiB | 15.1 MB |
| shell | 1 | 0.40 s | 0.27 s | 0.67 s | 0.35 s | 1.90x | 2.99 s | 3.38 s | 3.17 s | 1.07x | 99.9 MiB | 139.4 MiB | 3.7 MB |
| lua | 34 | 1.54 s | 0.97 s | 2.51 s | 1.35 s | 1.86x | 3.65 s | 5.19 s | 3.83 s | 1.36x | 35.3 MiB | 116.4 MiB | 3.9 MB |
| doom | 82 | 2.70 s | 1.93 s | 4.64 s | 2.58 s | 1.79x | 4.69 s | 7.39 s | 5.65 s | 1.31x | 25.1 MiB | 109.2 MiB | 5.5 MB |
| torture | 5 | 0.06 s | 0.17 s | 0.23 s | 0.18 s | 1.25x | 0.54 s | 0.60 s | 0.56 s | 1.07x | 15.0 MiB | 106.0 MiB | 0.9 MB |

## Frontend phases (seconds)

| Input | lex | preprocess | parse | sema | irgen | write |
|---|---|---|---|---|---|---|
| sqlite3 | 0.32 | 0.36 | 0.29 | 0.15 | 0.19 | 0.03 |
| shell | 0.11 | 0.16 | 0.05 | 0.03 | 0.04 | 0.01 |
| lua | 0.51 | 0.75 | 0.14 | 0.04 | 0.04 | 0.01 |
| doom | 0.99 | 1.15 | 0.26 | 0.10 | 0.06 | 0.01 |
| torture | 0.01 | 0.02 | 0.01 | 0.00 | 0.01 | 0.00 |
