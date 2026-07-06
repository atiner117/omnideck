<!-- One monochrome stroke glyph from $lib/icons.ts, sized like text (1em) and tinted by
     currentColor, so it drops in anywhere an emoji glyph used to sit. -->
<script lang="ts">
  import { ICONS, type IconName } from "./icons";

  let { name, size = "1em" }: { name: IconName; size?: string } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor"
  stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <!-- eslint-disable-next-line svelte/no-at-html-tags — static, app-defined path data -->
  {@html ICONS[name]}
</svg>
