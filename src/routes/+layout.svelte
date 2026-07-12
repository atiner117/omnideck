<script lang="ts">
  // Root layout: renders the page and mounts app-wide overlay chrome that must not live
  // in +page.svelte (the page owns XMB/nav; overlays that cover EVERYTHING — like the
  // OLED screensaver — sit here so they stack above any page-managed modal).
  import type { Snippet } from "svelte";
  import ScreensaverOverlay from "$lib/ScreensaverOverlay.svelte";

  let { children }: { children: Snippet } = $props();
</script>

{@render children()}
<ScreensaverOverlay />
