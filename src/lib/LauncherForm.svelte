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

  // ---- controller path ----
  // The form is reachable from the pad (Settings → "Add custom launcher"), so it must be
  // drivable without a keyboard — before this, the page's overlay entry routed only East
  // (close) and a pad-only user could open a form they could never fill or submit. The
  // page routes D-pad/stick up-down to padMove, South to padActivate, East to close.
  // Typing into the text fields still needs a physical keyboard (the hint says so), but
  // Cancel/Add/Category are always reachable, so the form is never a trap.
  const CATS = ["games", "video", "music", "apps"];
  const FIELDS = ["f-name", "f-exec", "f-icon", "f-cat", "cancel", "add"] as const;
  let padFocus = $state(-1); // -1 until the pad first moves — pointer/keyboard users never see it
  export function padMove(d: number) {
    const start = padFocus < 0 ? (d > 0 ? -1 : FIELDS.length) : padFocus;
    padFocus = Math.max(0, Math.min(FIELDS.length - 1, start + d));
    const id = FIELDS[padFocus];
    // Real DOM focus on the fields (the accent :focus border is the cursor); the buttons
    // show the pad cursor via .padfocus instead (:focus-visible won't fire for script focus).
    if (id.startsWith("f-")) document.getElementById(id)?.focus();
    else (document.activeElement as HTMLElement | null)?.blur?.();
  }
  export function padActivate() {
    const cur = padFocus >= 0 ? FIELDS[padFocus] : null;
    if (cur === "cancel") onclose();
    else if (cur === "add") addCustom();
    else if (cur === "f-cat") fCat = CATS[(CATS.indexOf(fCat) + 1) % CATS.length];
    else if (cur === null) padMove(1); // first South just lands on the first field
  }

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
    <button class="cbtn" class:padfocus={padFocus === 4} onclick={onclose}>Cancel</button>
    <button class="cbtn danger" class:padfocus={padFocus === 5} onclick={addCustom}>Add</button>
  </div>
  <p class="phint">Split on spaces; quote paths that contain them: "/My Games/app" --flag. Use the full path if it isn't on PATH. Esc/B to close. D-pad moves between fields, A cycles Category and presses the buttons — typing needs a keyboard.</p>
</Modal>

<style>
  .frow { display: flex; align-items: center; gap: 14px; margin: 8px 0; }
  .frow label { width: 96px; flex: 0 0 auto; color: #9fb0c8; font-weight: 600; font-size: clamp(13px, 1.3vw, 15px); }
  .frow input, .frow select { flex: 1; background: #0c1320; border: 1px solid #2c3a5c; color: #eef2f8; border-radius: 9px; padding: 9px 12px; font-size: clamp(13px, 1.4vw, 16px); }
  .frow input:focus, .frow select:focus { outline: none; border-color: var(--accent); }
  /* Pad cursor on the buttons (script focus doesn't trigger :focus-visible). */
  .confirm-btns .cbtn.padfocus { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
