# Releasing OmniDeck

The runbook for cutting a release. Versions are enforced in CI (`Cargo.toml` ==
`tauri.conf.json` == `packaging/PKGBUILD`; `package.json` and `Cargo.lock` ride along).

## 0. Pre-flight (already automated)

- `./packaging/test-session.sh` — the nested-gamescope harness must be 8/8.
- CI green on `main` (clippy + test, cargo-deny, cargo-audit, version-sync,
  ts-rs drift, AUR makepkg in a clean Arch container). Formatting is *not* a CI gate —
  the tree is hand-maintained in a compact style, not `cargo fmt` output.
- A real-hardware session pass for anything the harness can't see: display mode
  (grep `session display mode` in `~/.local/state/omnideck/omnideck.<date>.log`),
  Steam launch → focus return, suspend, controllers.

## 1. Version bump (one PR)

```bash
# all five together — CI's version-sync job rejects a partial bump
src-tauri/Cargo.toml        version = "X.Y.Z"
src-tauri/tauri.conf.json   "version": "X.Y.Z"
packaging/PKGBUILD          pkgver=X.Y.Z
package.json                "version": "X.Y.Z"
cd src-tauri && cargo check # refreshes Cargo.lock's own version
cd packaging && makepkg --printsrcinfo > .SRCINFO
```

Retitle the CHANGELOG's `## [Unreleased] — X.Y.Z` heading to `## [X.Y.Z] — YYYY-MM-DD`
in the same PR. Merge it.

## 2. Tag + GitHub release

```bash
git checkout main && git pull
git tag -a vX.Y.Z -m "OmniDeck X.Y.Z"
git push https://github.com/atiner117/omnideck.git vX.Y.Z
gh release create vX.Y.Z --title "OmniDeck X.Y.Z" --notes-file <(changelog section)
```

The tag is what materializes the source tarball the PKGBUILD points at
(`.../archive/refs/tags/vX.Y.Z.tar.gz`) — everything below depends on it existing.

## 3. Refresh PKGBUILD checksums (post-tag, tiny follow-up commit)

```bash
cd packaging
updpkgsums                          # downloads the tag tarball, rewrites b2sums
makepkg --printsrcinfo > .SRCINFO
# commit + push (the packaging CI job re-validates the real tarball this time)
```

## 4. Publish to the AUR

One-time setup: an AUR account with your SSH key
(https://wiki.archlinux.org/title/AUR_submission_guidelines).

```bash
git clone ssh://aur@aur.archlinux.org/omnideck.git aur-omnideck
cp packaging/PKGBUILD packaging/.SRCINFO packaging/omnideck.install aur-omnideck/
cd aur-omnideck && git add -A && git commit -m "omnideck X.Y.Z" && git push
```

Smoke-test from a clean chroot or another machine: `yay -S omnideck`.

## 5. After

- Verify the Forgejo pull-mirror picked up the tag (10-min interval).
- Install on the integrated-GPU host and run the session there (Mesa path).
- Open the next `## [Unreleased]` section in the CHANGELOG.
