#!/usr/bin/env bash
# Make Playwright's WebKit runnable on a rolling distro (Arch/CachyOS).
#
# WebKit is the engine that matters here: Tauri's Linux webview is WebKitGTK, so a WebKit
# screenshot is the one that shows what the app actually looks like. But Playwright ships a
# WebKit built on Ubuntu 24.04, linked against sonames a rolling distro moved past:
#
#     libicu{data,i18n,uc}.so.74   Arch is on ICU 78 (libicu*.so.78)
#     libxml2.so.2                 Arch is on libxml2.so.16
#     libflite*.so.1               not packaged for Arch at all
#
# Rather than install old libraries system-wide (they'd collide with the current ICU, and
# `pacman -U` an archived icu is a downgrade of a package half the system links against),
# this drops the .so files from Ubuntu 24.04's own packages into the WebKit bundle's private
# `sys/lib` — which its launcher already puts on LD_LIBRARY_PATH, alongside the libsoup /
# brotli / libjxl that Playwright bundles there for exactly the same reason.
#
# Nothing outside ~/.cache/ms-playwright is touched, no root is needed, and
# `bunx playwright install --force webkit` undoes it.
#
#   ./e2e/install-webkit-deps.sh          # install what's missing
#   ./e2e/install-webkit-deps.sh --force  # re-copy even if present
set -euo pipefail

FORCE=0
[ "${1:-}" = "--force" ] && FORCE=1

# Pinned to Ubuntu 24.04 (noble) — the release Playwright's WebKit is built on. Pinned, not
# "newest in the pool", because the pool holds every release's builds and lexical sort puts
# 2.9.4 above 2.9.14.
UBUNTU=http://archive.ubuntu.com/ubuntu/pool
DEBS=(
  "$UBUNTU/main/i/icu/libicu74_74.2-1ubuntu3.1_amd64.deb"
  "$UBUNTU/main/libx/libxml2/libxml2_2.9.14+dfsg-1.3ubuntu3.8_amd64.deb"
  "$UBUNTU/universe/f/flite/libflite1_2.2-6build3_amd64.deb"
)
# Only these leave the .debs — no headers, no binaries, no config. Each pattern has to match
# the real file as well as the soname symlink, or the symlink lands dangling: libflite's
# `libflite.so.1` points at `libflite.so.2.2`, hence the open-ended version glob there.
WANTED=("libicu*.so.74*" "libxml2.so.2*" "libflite*.so.*")

browsers_root="${PLAYWRIGHT_BROWSERS_PATH:-$HOME/.cache/ms-playwright}"
bundle=$(find "$browsers_root" -maxdepth 1 -type d -name 'webkit-*' 2>/dev/null | sort -V | tail -1)
if [ -z "$bundle" ]; then
  echo "error: no webkit bundle under $browsers_root — run: bunx playwright install webkit" >&2
  exit 1
fi
echo "webkit bundle: $bundle"

# The headless (wpe) and headed (gtk) browsers each have their own sys/lib.
targets=()
for flavour in minibrowser-wpe minibrowser-gtk; do
  [ -d "$bundle/$flavour/sys/lib" ] && targets+=("$bundle/$flavour/sys/lib")
done
[ ${#targets[@]} -gt 0 ] || { echo "error: no sys/lib in $bundle" >&2; exit 1; }

if [ "$FORCE" -eq 0 ] && [ -e "${targets[0]}/libicuuc.so.74" ] \
   && [ -e "${targets[0]}/libxml2.so.2" ] && [ -e "${targets[0]}/libflite.so.1" ]; then
  echo "already installed (--force to redo)"
else
  work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
  for url in "${DEBS[@]}"; do
    deb="$work/$(basename "$url")"
    echo "fetching $(basename "$url")"
    curl -fsSL --retry 3 -o "$deb" "$url"
    # A .deb is an `ar` archive holding data.tar.{xz,zst}; tar sniffs the compression.
    ( cd "$work" && ar x "$deb" && tar xf data.tar.* && rm -f data.tar.* control.tar.* debian-binary )
  done

  libdir="$work/usr/lib/x86_64-linux-gnu"
  for target in "${targets[@]}"; do
    for pattern in "${WANTED[@]}"; do
      # -a keeps the soname symlinks (libicuuc.so.74 -> libicuuc.so.74.2) intact.
      find "$libdir" -maxdepth 1 -name "$pattern" -exec cp -a {} "$target/" \;
    done
  done
  echo "installed into: ${targets[*]}"
fi

# Prove it: resolve the headless browser's libraries the way its launcher does.
for flavour in minibrowser-wpe minibrowser-gtk; do
  bin="$bundle/$flavour/bin/MiniBrowser"
  [ -x "$bin" ] || continue
  missing=$(LD_LIBRARY_PATH="$bundle/$flavour/lib:$bundle/$flavour/sys/lib" \
    ldd "$bin" | grep 'not found' | awk '{print $1}' | sort -u || true)
  if [ -n "$missing" ]; then
    echo "STILL MISSING ($flavour):" >&2
    echo "$missing" >&2
    exit 1
  fi
  echo "ok: $flavour resolves all libraries"
done

echo "run it: bun run screenshots:webkit"
