---
title: "install-session.sh: honor SDDM's effective Wayland SessionDir"
kind: walkthrough
status: done
created: 2026-10-14
updated: 2026-10-14
thread: claude-code:omnideck-sddm-sessiondir (branch fix/install-session-sddm-sessiondir)
---

## Problem

CachyOS ships `/usr/lib/sddm/sddm.conf.d/general.conf` with `[Wayland]
SessionDir=/usr/share/wayland-sessions`. install-session.sh only wrote
`/usr/local/share/wayland-sessions`, so the greeter never listed OmniDeck and autologin
failed ("Unable to find autologin session entry"). Reproduced on ares, whose live config
resolves to `/usr/share/wayland-sessions` only.

## Change areas

1. `packaging/install-session.sh`: shell functions `sddm_conf_files` (SDDM's read order,
   per `man 5 sddm.conf`: /usr/lib/sddm/sddm.conf.d/*.conf → /etc/sddm.conf.d/*.conf →
   /etc/sddm.conf, C-sorted), `sddm_wayland_session_dir_value` (last `[Wayland] SessionDir=`
   wins; other sections and comments ignored), `sddm_session_dirs` (split on commas, trim,
   unquote, strip trailing `/`, dedupe; empty/unset → SDDM's compiled-in default),
   `session_link_candidates`. The entry is still written to /usr/local (GDM, immutable
   distros); when SDDM's list omits it, a symlink goes into the first listed dir that
   accepts it, else a warning prints the `/etc/sddm.conf.d` drop-in.
2. `--print-session-dirs` (no root) and `OMNIDECK_SDDM_ROOT` (fixture prefix); `--help`;
   unknown `-flags` rejected instead of being treated as the binary path.
3. `packaging/test-install-session.sh` (18 checks over 9 fixture trees) + CI job
   `install-session` in packaging.yml.
4. `packaging/README.md` (new) documents the resolution, the drop-in override and autologin.

## Gotchas

- `ln -sfn` replaces an existing regular `omnideck.desktop` in the SDDM dir (ares has a
  hand-installed copy from Jun 23 in /usr/share/wayland-sessions); that is intended — one
  source of truth.
- If both GDM and a non-default SDDM list exist, GDM may list the entry twice (/usr/local
  + the link in /usr/share). Accepted edge case.
- `/etc/sddm.conf` is read after /etc/sddm.conf.d, so a SessionDir there overrides the
  documented drop-in.

## Verify

```bash
./packaging/test-install-session.sh
./packaging/install-session.sh --print-session-dirs
```
The real install path was exercised in an `unshare -rm` namespace with bind-mounted
/usr/local/share/wayland-sessions, /usr/local/bin, /usr/share/wayland-sessions and a
passthrough `sudo`: link created on CachyOS config; read-only /usr/share → warning path.
Couch: on r2d2, remove the SessionDir line from zz-autologin.conf, rerun the installer,
restart SDDM, confirm autologin lands in OmniDeck.
