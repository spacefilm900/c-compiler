# Releasing hcc

Each step needs the owner's approval; nothing here runs on its own.

1. Make sure `main` is green and that `Cargo.toml`, `CHANGELOG.md` and
   `docs/release-notes/vX.Y.Z.md` all name the new version.
2. Tag the merge commit and push the tag:

   ```sh
   git tag -a vX.Y.Z -m "hcc X.Y.Z" <commit>
   git push origin vX.Y.Z
   ```

3. The `release` workflow (`.github/workflows/release.yml`) builds and
   smoke-tests the four archives. It checks that the tag matches
   `Cargo.toml` and uploads one `release-bundle` artifact: the archives and
   `SHA256SUMS`. The workflow has read-only repository access and does not
   create the release.
4. Download the bundle, check it, and create the release from it:

   ```sh
   gh run download <run-id> --name release-bundle --dir dist
   (cd dist && sha256sum -c SHA256SUMS)
   gh release create vX.Y.Z --verify-tag --draft --title "hcc X.Y.Z" \
     --notes-file docs/release-notes/vX.Y.Z.md dist/*.tar.gz dist/*.zip dist/SHA256SUMS
   ```

5. Read the draft on GitHub, then publish it.
