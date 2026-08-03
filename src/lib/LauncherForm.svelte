<!-- Add-custom-launcher form. Owns its field drafts (the page {#if}-mounts it, so each
     open starts fresh — same net behavior as the old explicit reset) and the slugify /
     de-dup / exec-split logic; hands the built App back via onadd. The page owns
     persistence (cfg update + save_apps) and the collision toast. Button/hint styling
     comes from the shared modal vocabulary in Modal.svelte; .frow is form-specific and
     lives here. -->
<script lang="ts">
  import Modal from "./Modal.svelte";
  import { splitArgv } from "./argv";
  import type { App } from "./backend";

  let {
    apps,
    onadd,
    onerror,
    onclose,
  }: {
    apps: App[];
    onadd: (app: App, collided: boolean) => void;
    onerror: (ctx: string, e: unknown) => void;
    onclose: () => void;
  } = $props();

  let fName = $state("");
  let fExec = $state("");
  let fIcon = $state("🚀");
  let fCat = $state("apps");

  function addCustom() {
    const name = fName.trim();
    const cmd = fExec.trim();
    if (!name || !cmd) { onclose(); return; }
    // Slugify, trimming leading/trailing dashes so "My App!" and "My App?" don't both collapse
    // to "custom-my-app-"; reject a name with no usable characters.
    const base = "custom-" + name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
    if (base === "custom-") { onerror("Add a name with at least one letter or number", null); return; }
    // De-dup with a numeric suffix instead of silently overwriting an existing same-slug launcher.
    const collided = apps.some((a) => a.id === base);
    let id = base; for (let n = 2; apps.some((a) => a.id === id); n++) id = `${base}-${n}`;
    // A bare URL (e.g. a SearXNG instance) is launched as a browser app so it opens in the
    // browser AND gets its site favicon; anything else is run as a normal argv command.
    // The split is quote-aware (review #6 / PR #24) so paths with spaces work:
    // "/My Games/app" --flag.
    const isUrl = /^https?:\/\//i.test(cmd);
    const argv = isUrl ? null : splitArgv(cmd);
    if (!isUrl && (!argv || argv.length === 0)) {
      onerror(argv ? "Command is empty" : "Unbalanced quote in command", null);
      return; // keep the form open so the user can fix it
    }
    const exec = isUrl ? ["BROWSER", `--app=${cmd}`] : argv!;
    onadd({ id, name, icon: fIcon || "🚀", exec, accent: "#3a4256", category: fCat }, collided);
  }
</script>

<Modal labelledby="dlg-form" backdropLabel="Close" {onclose}>
  <h2 id="dlg-form">Add custom launcher</h2>
  <div class="frow"><label for="f-name">Name</label><input id="f-name" bind:value={fName} placeholder="My App" /></div>
  <div class="frow"><label for="f-exec">Command</label><input id="f-exec" bind:value={fExec} placeholder="/usr/bin/foo --flag" /></div>
  <div class="frow"><label for="f-icon">Icon</label><input id="f-icon" bind:value={fIcon} placeholder="🚀" /></div>
  <div class="frow"><label for="f-cat">Category</label>
    <select id="f-cat" bind:value={fCat}>
      <option value="games">Games</option>
      <option value="video">Movies &amp; TV</option>
      <option value="music">Music</option>
      <option value="apps">Apps</option>
    </select>
  </div>
  <div class="confirm-btns">
    <button class="cbtn" onclick={onclose}>Cancel</button>
    <button class="cbtn danger" onclick={addCustom}>Add</button>
  </div>
  <p class="phint">Split on spaces; quote paths that contain them: "/My Games/app" --flag. Use the full path if it isn't on PATH. Esc to close.</p>
</Modal>

<style>
  .frow { display: flex; align-items: center; gap: 14px; margin: 8px 0; }
  .frow label { width: 96px; flex: 0 0 auto; color: #9fb0c8; font-weight: 600; font-size: clamp(13px, 1.3vw, 15px); }
  .frow input, .frow select { flex: 1; background: #0c1320; border: 1px solid #2c3a5c; color: #eef2f8; border-radius: 9px; padding: 9px 12px; font-size: clamp(13px, 1.4vw, 16px); }
  .frow input:focus, .frow select:focus { outline: none; border-color: var(--accent); }
</style>
