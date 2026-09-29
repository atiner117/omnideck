---
title: "Recent-app task overview UX and tests"
kind: ticket
status: done
created: 2026-09-28
updated: 2026-09-28
thread: t3code/0075e72c
---

Goal: iPad/Android-like recent-app overview for controller, with Home and running app cards. In scope: MRU order for actual owned running apps, Home selection, explicit card close, empty state, focus/accessibility, input help, Svelte/unit and nested-session tests. Out of scope: Steam game kill/focus, thumbnails. Depends on `input-and-state` and spec §Product contract. Likely files: `src/routes/+page.svelte`, `src/lib/DeckSwitcher.svelte`, `src/lib/backend.ts`, `packaging/test-session.sh`, help docs. Acceptance: hold opens overview, A selects, B restores pre-open foreground, Home card lands Home, close one card leaves others, empty overview usable, reduced motion honored, keyboard and controller flows covered.
