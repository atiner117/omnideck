<!-- Controls reference — the full keyboard/controller table that used to crowd the footer.
     Purely presentational; the page routes input (Esc/◯ close) and owns the open state. -->
<script lang="ts">
  import Modal from "./Modal.svelte";

  let { inSession, onclose }: { inSession: boolean; onclose: () => void } = $props();

  const rows: Array<[string, string, string]> = [
    ["Navigate", "← → ↑ ↓", "D-pad / left stick"],
    ["Select / launch", "Enter", "✕"],
    ["Back / close", "Esc", "◯"],
    ["Favorite", "F", "□"],
    ["Add apps & media", "A", "△"],
    ["Search", "/", "Select"],
    ["Item info", "I", "R1"],
    ["Now Playing controls", "N", "L1"],
    ["Home", "H", "Start"],
    ["Settings", "P", "—"],
    ["Help", "? / F1", "—"],
  ];
  const sessionRows: Array<[string, string, string]> = [
    ["Switch app ⇄ OmniDeck", "Ctrl+Alt+Home", "Guide press"],
    ["Close the running app", "Ctrl+Alt+End", "Guide hold"],
  ];
</script>

<Modal labelledby="dlg-help" backdropLabel="Close help" closeLabel="Close help" {onclose}>
  <h2 id="dlg-help">Controls</h2>
  <div class="helpgrid">
    <span class="hhead"></span><span class="hhead">Keyboard</span><span class="hhead">Controller</span>
    {#each rows as [action, kbd, pad] (action)}
      <span class="haction">{action}</span><span class="hkey">{kbd}</span><span class="hkey">{pad}</span>
    {/each}
    {#if inSession}
      <span class="hsect">While an app is running</span>
      {#each sessionRows as [action, kbd, pad] (action)}
        <span class="haction">{action}</span><span class="hkey">{kbd}</span><span class="hkey">{pad}</span>
      {/each}
    {/if}
  </div>
  <p class="phint">Esc/◯ close</p>
</Modal>

<style>
  .helpgrid { display: grid; grid-template-columns: 1fr auto auto; gap: 7px 26px; margin: 4px 0 8px; align-items: baseline; }
  .hhead { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 1px; font-weight: 700; }
  .hsect { grid-column: 1 / -1; color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 2px; font-weight: 700; padding-top: 10px; }
  .haction { color: #dde5f0; font-size: clamp(13px, 1.4vw, 16px); font-weight: 600; }
  .hkey { color: var(--accent); font-size: clamp(12px, 1.3vw, 15px); font-weight: 700; white-space: nowrap; }
</style>
