<script lang="ts">
  // Root layout: renders the page and mounts app-wide overlay chrome that must not live
  // in +page.svelte (the page owns XMB/nav; overlays that cover EVERYTHING — like the
  // OLED screensaver — sit here so they stack above any page-managed modal).
  import type { Snippet } from "svelte";
  // Self-hosted Inter (@font-face) — the family +page.svelte asks for. Imported before the
  // themes so the face is declared by the time anything paints.
  import "$lib/fonts.css";
  // Global (unscoped) theme token overrides — see src/lib/themes/themes.ts.
  import "$lib/themes/themes.css";
  import ScreensaverOverlay from "$lib/ScreensaverOverlay.svelte";

  let { children }: { children: Snippet } = $props();
</script>

{@render children()}
<ScreensaverOverlay />
