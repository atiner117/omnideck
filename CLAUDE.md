# CLAUDE.md — omnideck

A **controller-first, 10-foot media & game launcher for Linux**: Tauri 2 shell (SvelteKit +
Svelte 5 frontend, Rust backend in `src-tauri/`), gamepad-driven, targeting an OLED TV inside a
gamescope session. Package manager is **Bun**. Repo mirrors to GitHub `atiner117/omnideck` (public,
GPL-3.0); Forgejo copy is read-only.

> **New here? Read `VISION.md` (the compass) and `NOTES-DEEPDIVE-ROADMAP.md` (the backlog) before
> choosing work.** This file is the always-on operating manual: how to run, how to verify, and the
> rules that keep the tree green.

## Golden rules (these bite if ignored)
- **Never touch `main`.** Branch, open a *draft* PR, let Andrew review + merge. No direct commits,
  no force-push, no merging your own PR.
- **The CI in `.github/workflows/ci.yml` IS the definition of done** — see [Verify](#verify) below.
  A PR isn't "ready" until every check it would run is green locally.
- **Rust structs are the source of truth for the TS types** (ts-rs). Change a `#[ts(export)]`
  struct → regenerate `src/lib/bindings/` or CI fails. Command is in Verify.
- **Five version strings must agree** (`Cargo.toml`, `tauri.conf.json`, `package.json`,
  `packaging/PKGBUILD`, `Cargo.lock`) — bump together per `RELEASING.md`. CI enforces it.
- **Respect the quality bars**: `NOTES-A11Y.md` (reduced-motion), `NOTES-PERFORMANCE.md` (never jank
  the gamepad rAF/input thread), `NOTES-SECURITY.md` (no plaintext secrets). A regression there is
  not shippable.
- **No AI co-author trailers** in commits (a `commit-msg` hook strips them; keep the contributors
  list clean).

## Run it

System deps (Tauri 2 / WebKitGTK 4.1) are listed in `README.md` → **Requirements**; on Arch/CachyOS
that's `webkit2gtk-4.1` + `libudev` (gamepad). Then:

```bash
bun install
bun run tauri dev                # dev: opens the app window, HMR frontend on :1420
bun run tauri build --no-bundle  # release binary -> src-tauri/target/release/omnideck
bun run dev                      # frontend ONLY in a browser (:1420) — fast UI iteration, no Rust/webview
```

Three ways to run the built app (detail in `README.md` → *Three ways to run it*):
1. **Desktop window** — `omnideck` (everything except session-only pieces: switcher, global chords).
2. **Big Picture** — `gamescope -f -- omnideck` (full 10-foot experience inside your desktop; needs
   nested-capable gamescope — on X11 it silently falls back to headless if built without SDL).
3. **Dedicated session** — `sudo ./packaging/install-session.sh`, then pick *OmniDeck* at the DM.

## Verify

**This is the part that unblocks "I haven't had time to test."** Most of the app is verifiable by an
agent *without a TV, controller, or logging out*. Do as much of this as applies to what you touched;
everything you run must pass.

### 1. Fast local gate (run on every change)
```bash
bun run check                 # svelte-check typecheck
bun run build                 # frontend build must succeed
bun run test                  # vitest — unit tests in src/**/*.test.ts (nav, osk, launchId)
# Rust (from src-tauri/):
cargo clippy --release --all-targets -- -D warnings
cargo test --release
```
Quicker Rust inner loop while iterating: `cargo check --manifest-path src-tauri/Cargo.toml`.

### 2. ts-rs bindings sync (whenever a Rust type changed)
```bash
cd src-tauri
TS_RS_EXPORT_DIR="$PWD/../src/lib/bindings" cargo test --release export_bindings
cd .. && git diff --exit-code src/lib/bindings   # must be clean; commit regenerated bindings
```

### 3. Headless CLI surface — exercise the backend, no GUI (`src-tauri/src/cli.rs`)
No window opens; great for verifying real logic against the real machine:
```bash
cargo run -- probe        # capability/tier + GPU/KMS/Vulkan detection (human + JSON)
cargo run -- scan         # Steam library scan
cargo run -- config       # resolved config: path + settings + apps
cargo run -- catalog      # bundled app/media catalog
cargo run -- media        # snapshot MPRIS players (what Now Playing would show)
cargo run -- mediasrv     # probe the configured media server (sections + first library)
cargo run -- mpvprofiles  # report the auto-generated mpv profile set
cargo run -- gridart 570  # fetch SteamGridDB art for an appid (needs steamgriddb_key)
cargo run -- bgprep <img> # wallpaper downscale-to-cache
```

### 4. Input/session paths without a physical pad or logout
The switcher / hotkey / watchdog / gamepad logic has a real test harness — this is the "couch test"
made automatable:
```bash
# Virtual Xbox pad over /dev/uinput (gilrs sees it as real hardware; needs `input` group):
cargo run --example virtual-pad -- guide-short          # Guide press < hold threshold
cargo run --example virtual-pad -- guide-hold 1000      # Guide hold (close-all)
cargo run --example virtual-pad -- stick-up 300         # left stick nav

# Full nested-gamescope pre-flight (7 real tests: boot paints non-black, Ctrl+Alt+Home
# hide/show, Ctrl+Alt+End close, pad deck/pick/close). Needs a desktop + nested-capable gamescope:
./packaging/test-session.sh
```
The FIFO control channel behind the harness lives in `src-tauri/src/testhook.rs`
(`OMNIDECK_TEST_CONTROL`, test builds only — never set in a real session).

### 5. What still needs a human on the couch (can't be automated)
Feel/latency, overscan + refresh-rate on the actual TV, controller ergonomics, and 10-foot visual
polish. Capture those findings in a dated `NOTES-COUCHTEST-*.md`. Everything above should be green
*before* it's worth your couch time — that's the whole point of the harness.

### Full CI parity before marking a PR ready
Beyond §1–2, CI also runs `bun run tauri build --no-bundle`, `cargo deny check` (from src-tauri),
`cargo audit`, and the five-way version-sync. Run these when the change could plausibly affect them
(new deps, bundling, version bumps).

## Architecture / module map

**Frontend** (`src/lib/`): `+page.svelte` (the grid) · `*Modal.svelte` (Catalog/Media/Search/Help) ·
`NowPlaying.svelte` · `Wizard.svelte` · `backend.ts` (IPC to Rust) · `nav.ts`/`osk.ts`/`launchId.ts`
(pure logic, unit-tested) · `bindings/` (generated — do not hand-edit).

**Rust** (`src-tauri/src/`):
- *Input & session:* `gamepad.rs`, `hotkey.rs`, `navpad.rs` (uinput kbd/mouse), `switcher.rs`,
  `session.rs`, `watchdog.rs`, `proc.rs`, `testhook.rs`.
- *Launch & library:* `apps.rs`, `commands.rs` (Tauri IPC), `library.rs` (Steam VDF/ACF scan),
  `capability.rs`, `gpu.rs`.
- *Media:* `mpris.rs` (zbus Now Playing), `media_server.rs`, `media_profiles.rs`, `http.rs`.
- *Art & assets:* `asset.rs`, `icons.rs`, `steamgriddb.rs`, `background.rs`.
- *Core:* `config.rs` (`~/.config/omnideck/config.toml`), `cli.rs`, `logging.rs`, `sync.rs`
  (ts-rs export), `lib.rs`, `main.rs`.

## Workflow & where to look
- `VISION.md` — north star (Andrew curates; the loop only reads it).
- `NOTES-DEEPDIVE-ROADMAP.md` — primary backlog; `NOTES-DEEPDIVE-{FRONTEND-SPLIT,MEDIA-SERVER}.md` —
  in-flight architecture tracks; `NOTES-ARCHITECTURE.md`/`NOTES.md` — orientation.
- `docs/night-log.md` — append-only journal of autonomous iterations (newest on top).
- `.claude/commands/omnideck-night.md` — the `/omnideck-night` loop: one small, green, reviewable
  increment per iteration, driven by `/loop`. Use it for "fire a prompt and check back" work.
- Offload bounded mechanical sub-work (bulk edits, large-diff summaries) to the local model via the
  `delegate` skill to conserve usage; keep judgment on Claude.

## Git & PR conventions
`origin` is SSH (`git@github.com:atiner117/omnideck.git`) and needs a **yubikey touch** — agents
can't push through it unattended. Push over the gh-token HTTPS helper instead (this does *not* change
`origin`):
```bash
BR="loop/night-$(date +%Y%m%d)"   # or feat/<topic>; one branch per line of work, off up-to-date main
git -c credential.helper='!gh auth git-credential' \
    push https://github.com/atiner117/omnideck.git "HEAD:refs/heads/$BR"
gh pr create --draft --base main --head "$BR" --title "<title>" --body "<what + verify results>"
```
Draft PR, green, logged in `docs/night-log.md` if it was a loop iteration — then stop. Andrew merges.
