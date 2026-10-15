# packaging/

| File | What it is |
|---|---|
| `PKGBUILD`, `.SRCINFO`, `omnideck.install` | AUR package (validated by `.github/workflows/packaging.yml`) |
| `install-session.sh` | Installs OmniDeck as a selectable gamescope session in the display manager |
| `test-install-session.sh` | Unit test for `install-session.sh`'s SDDM config resolution (no root) |
| `test-session.sh` | Nested-gamescope session pre-flight (see `M2-SESSION-TEST.md`) |
| `test-profiles.sh`, `bench-profiles.sh` | mpv/VapourSynth profile validation and benchmarks |

## Dedicated session: `install-session.sh`

```bash
sudo ./packaging/install-session.sh [/path/to/omnideck]   # AUR: /usr/share/omnideck/install-session.sh
```

It writes:

- `/usr/local/bin/omnideck-session`: the gamescope launcher (per-host flags go in
  `~/.config/omnideck/session.conf`).
- `/usr/local/share/wayland-sessions/omnideck.desktop`: the session entry. GDM finds it
  through `XDG_DATA_DIRS`, and SDDM's built-in default scans it. It stays writable on
  immutable distros (Bazzite/SteamOS) where `/usr/share` is read-only.
- **Only when SDDM does not scan that directory:** a symlink to the entry in the first
  directory SDDM does scan (see below).

### SDDM's `SessionDir`

SDDM looks for Wayland sessions in the comma-separated `[Wayland] SessionDir=` list. The
default is `/usr/local/share/wayland-sessions,/usr/share/wayland-sessions`, but distros
override it. CachyOS ships `/usr/lib/sddm/sddm.conf.d/general.conf` with
`SessionDir=/usr/share/wayland-sessions` only. An entry in `/usr/local` is then
invisible to the greeter, and autologin fails with `Unable to find autologin session entry`.

The installer resolves the *effective* value the way SDDM does (`man 5 sddm.conf`):
`/usr/lib/sddm/sddm.conf.d/*.conf`, then `/etc/sddm.conf.d/*.conf` (each sorted by name),
then `/etc/sddm.conf`. A later file wins per key. `sddm --example-config` is no help here:
it prints the compiled-in defaults, not what your config files set.

To see what the installer would do, without root:

```bash
./packaging/install-session.sh --print-session-dirs
# config      /usr/lib/sddm/sddm.conf.d/general.conf
# sessiondir  /usr/share/wayland-sessions
# install     /usr/local/share/wayland-sessions/omnideck.desktop
# link        /usr/share/wayland-sessions/omnideck.desktop
```

A `link` line means the canonical directory is not on SDDM's list, so the entry will
also be symlinked there. The installer falls back to the next listed directory if one
refuses the link (for example, read-only `/usr/share`).

**Alternative: put `/usr/local` back on SDDM's list.** Use this when no listed directory
is writable, or when you would rather not have a link in `/usr/share`. Files in
`/etc/sddm.conf.d` override the distro's `/usr/lib/sddm/sddm.conf.d`. `/etc/sddm.conf`
is read last and would override this drop-in if it also sets `SessionDir`:

```ini
# /etc/sddm.conf.d/zz-omnideck-sessiondir.conf
[Wayland]
SessionDir=/usr/local/share/wayland-sessions,/usr/share/wayland-sessions
```

Autologin into the session uses the entry's file name:

```ini
# /etc/sddm.conf.d/zz-autologin.conf
[Autologin]
User=<you>
Session=omnideck
```

Restart SDDM (`sudo systemctl restart display-manager`) or reboot after editing either file.

To uninstall, remove the files the installer printed in its `To remove:` line:
`/usr/local/share/wayland-sessions/omnideck.desktop`, `/usr/local/bin/omnideck-session`,
and the symlink, if one was made.

### Testing

```bash
./packaging/test-install-session.sh   # fake SDDM config trees → --print-session-dirs
```

The test runs in CI (`packaging.yml`, job `install-session`) for every change under
`packaging/`. `OMNIDECK_SDDM_ROOT=<dir>` makes the installer read SDDM's config paths
under `<dir>`. That is how the test feeds it fixtures, and you can use it to check a copy
of another machine's `/etc` and `/usr/lib/sddm`.
