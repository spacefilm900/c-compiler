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

Please report a vulnerability privately, not in a public issue. Use
GitHub's private vulnerability reporting: on the repository's **Security**
tab, choose **Report a vulnerability**. Include the input, the command line
and `hcc --version`.

Security fixes are made for the supported versions above (1.0.x). hcc is
maintained by one person, so responses are best effort: there is no fixed
response time. A fix goes into the next patch release, and the advisory
credits you unless you ask otherwise.

## Verifying a release

Each release lists a SHA-256 checksum for every archive in `SHA256SUMS`,
and each archive has its own `.sha256` file. Check them before use:

```sh
sha256sum -c SHA256SUMS --ignore-missing     # Linux
shasum -a 256 -c SHA256SUMS --ignore-missing # macOS
```

Check that an `OK` line names the archive you downloaded: with
`--ignore-missing`, a run that finds only other archives checks nothing
you will use.

On Windows, in PowerShell, compare the printed hash with the archive's line
in `SHA256SUMS`:

```powershell
Get-FileHash -Algorithm SHA256 hcc-1.0.1-windows-x86_64.zip
```
