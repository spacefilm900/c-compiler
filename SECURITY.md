# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |
| Earlier, unversioned builds | No |

## What counts

hcc reads C source and writes LLVM IR. Please report privately:

- a crash, hang or unbounded memory use caused by an input file, where
  that could matter to someone compiling untrusted code (for example in a
  build service);
- a miscompilation that silently removes or weakens a check the source
  makes (bounds, overflow, a security-relevant comparison);
- anything in the release archives or the release workflow that could let
  someone tamper with a published binary.

An ordinary wrong-code bug or a rejected valid program is a normal bug:
open an issue (see [CONTRIBUTING.md](CONTRIBUTING.md)).

hcc runs no code from the program it compiles. `--link` runs Clang (or
`$HCC_CLANG`) with the arguments you give, so treat `HCC_CLANG` and the
command line as trusted.

## Reporting

Contact the maintainer, Keith Haviland, privately through GitHub (a
private security advisory on the repository, where that is enabled), not
in a public issue. Please include the input, the command line and
`hcc --version`. You should get a first answer within a week. A fix goes
into the next patch release, and the advisory credits you unless you ask
otherwise.

## Verifying a release

Each release lists a SHA-256 checksum for every archive in `SHA256SUMS`,
and each archive has its own `.sha256` file. Check them before use:

```sh
sha256sum -c SHA256SUMS --ignore-missing     # Linux
shasum -a 256 -c SHA256SUMS --ignore-missing # macOS
```

On Windows, in PowerShell, compare the printed hash with the archive's line
in `SHA256SUMS`:

```powershell
Get-FileHash -Algorithm SHA256 hcc-1.0.0-windows-x86_64.zip
```
