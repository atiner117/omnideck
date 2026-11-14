# NOTES-A11Y.md — accessibility (2026-06-27)

> OmniDeck is controller-first on a TV, so the primary paths (gamepad + arrow keys) work — but
> keyboard-Tab users, screen-reader users, and vestibular-sensitive users are poorly served, and
> several gaps are WCAG-scoped. All verified at HEAD `1f1d617`. The single highest-leverage fix
> is the `<Modal>` component extraction ([[NOTES-FRONTEND.md]] §6), which knocks out items 1+2+3
> together.

## 1. No dialog semantics / focus-trap on any of the 6 modals — `medium` · `open`
All six are plain `<div class="prefs …">`: search `:985`, catalog `:1014`, info `:1038`, power
`:1070`, confirm `:1087`, form `:1100`. Grep confirms **zero** `role="dialog"`/`aria-modal`/
`aria-labelledby`/`tabindex`/focus-trap in the file (the only `role=`s are `role="group"` on the
OSK `:1002` and `role="alert"` on the toast `:1174`). Esc-to-close is wired (`onKey`/`onGamepad`),
but focus is never moved into the dialog on open or trapped, so Tab escapes into the XMB behind
the backdrop. WCAG 2.4.3 / 4.1.2 (modal pattern).

**Action:** on each container add `role="dialog" aria-modal="true" aria-labelledby=<h2 id>`;
on open, `focus()` the first control (`.prefs-close`) and restore to the opener on close. The
clean fix is the `<Modal>` component. (Note: the close `✕` buttons use `title=`, not
`aria-label`; the backdrops + OSK group do carry `aria-label`.)

## 2. Modal rows are mouse-only `<div onclick>` — `low` · `open` (downgraded from medium)
`<div class="crow" … onclick=… onmouseenter=…>` at `:991,997,1024,1075` — no `role`, no
`tabindex`, no per-row key handler. Invisible to Tab and to screen readers (4.1.2 Name/Role/Value).
Downgraded to `low` because the global `keydown` handler (`:756`) **does** activate every row via
arrows+Enter with a visible `.focused` ring — so operable-from-keyboard (2.1.1) is met; the real
gap is Tab-focus convenience + SR semantics.

**Action:** swap each `<div class="crow">` → `<button class="crow">` (free focusability +
Enter/Space; gives the §1 focus-trap a real first/last target). The existing arrow-nav +
`class:focused` keep working unchanged.

## 3. Dropped focus outline on inputs — `low` · `open`
`+page.svelte:1242` `.numedit:focus, .textedit:focus { outline: none; }` — and there's no
`:focus-visible` rule anywhere in the stylesheet. The numedit has a static accent border that
doesn't change on focus, so a keyboard user gets no "I'm focused here" signal (contrast `.frow`
inputs `:1306` which at least swap border-color).

**Action:** add app-wide `:focus-visible` (keyboard-only, not mouse):
```css
.numedit:focus-visible, .textedit:focus-visible, .cbtn:focus-visible, .oskkey:focus-visible {
  outline: 2px solid var(--accent); outline-offset: 1px;
}
```

## 4. Header / now-playing buttons rely on `title` only — `low` · `open`
Header (`:912-915`): 🔍 / ＋ / ⚙ / ⏻ each have `title=` but no `aria-label` — `title` is an
unreliable accessible name (ignored by many SR configs, never on touch), so the name is the raw
emoji glyph. Same for the now-playing controls (`:1161-1163` ⏮ ⏸ ⏭) and dismiss (`:1167`).

**Action:** add `aria-label="Search"` / `"Add apps"` / `"Settings"` / `"Power menu"` /
`"Previous"` / `"Play/Pause"` / `"Next"` / `"Close & return"` / `"Dismiss"`.

## 5. No `prefers-reduced-motion` — `low` · `open`
Infinite spinner (`:1251`, `np-spin 0.9s linear infinite`), EQ bars (`:1255`), and many
`transition:`s — no `@media (prefers-reduced-motion)` anywhere. Meaningful for
vestibular-sensitive users on a living-room display.

**Action:**
```css
@media (prefers-reduced-motion: reduce) {
  .np-spinner, .np-eq i { animation: none; }
  .xcats, .xitems, .xbg, .xitem { transition: none !important; }
}
```
(consider a static glyph for the spinner state).

## 6. Footer contrast < AA — `low` · `open`
`footer` (`:1321`) `color: #5b6678` on near-black is ~3.5:1; WCAG AA needs 4.5:1 for small text,
and the footer is also the only keybind legend (carries real info). Spot-check `.ccat #6b7790`
(`:1277`) and `.phint #7e8aa0` (`:1289`) at their font sizes too.

**Action:** raise footer to ~`#8a96ab` (the `.xempty` tone, ~5.4:1) or `#93a0b6` (the footer
`<b>` tone) to clear AA.

## 7. Global `e.preventDefault()` on arrows disables native focus traversal — `low` · `open`
`onKey` (`:693`) preventDefaults arrow keys app-wide, so native button focus-order is suppressed.
Intended for the custom XMB ring, but it's why Tab/arrow behave oddly for AT users — reinforces
the §1/§2 fixes (give modals/rows real focus targets and a trap).

## What's already good
- Error toast has `role="alert" aria-live="assertive"` (`:1174`) — screen readers hear failures.
- OSK has `role="group" aria-label` (`:1002`).
- All interpolation is text/`style=` — no `{@html}`/`innerHTML` XSS sinks.

**Cross-refs:** [[NOTES-FRONTEND.md]] §6 (`<Modal>` extraction fixes 1+2+3), [[NOTES-PUBLIC-RELEASE.md]]
(a11y baseline as a 0.2.0 gate), [[NOTES.md]] (roadmap #7).
