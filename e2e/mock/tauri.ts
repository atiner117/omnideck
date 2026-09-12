// Stands in for the Rust backend so the SvelteKit frontend can run in a plain browser
// (`bun run dev`) with no Tauri webview, no window, and no machine state.
//
// `@tauri-apps/api` is a thin shim: `invoke()` is `window.__TAURI_INTERNALS__.invoke(cmd, args)`
// and `listen()` is `invoke("plugin:event|listen", …)` with a callback id from
// `transformCallback`. Installing our own `__TAURI_INTERNALS__` before any app script runs
// therefore intercepts the whole IPC surface without touching production code.
//
// Two fidelity gaps are inherent to the browser and worth knowing when reading a screenshot:
//   • `omnideck://` art (Steam box art, media posters, cached wallpapers) is a Tauri custom
//     protocol — the browser can't resolve it, so the UI shows its name-tile fallback.
//   • `app_icon` favicons are fetched by the backend over the network; the mock returns none
//     by default, so app tiles render emoji-on-accent.
// Everything else — layout, nav, modals, settings, media browse — is the real code path.
import type { Page } from "@playwright/test";
import { type Backend, defaultBackend } from "./fixtures";

/** Test-side handle the mock exposes on `window` (see `emit` / `unmocked`). */
export type MockHandle = {
  /** Deliver a backend event (`app-exited`, `gamepad-event`, `guide-tap`, …); returns the listener count. */
  emit(event: string, payload: unknown): number;
  /** Commands the app called that the mock has no handler for — should stay empty. */
  unmocked: string[];
  /** Every command name invoked, in order. */
  calls: string[];
};

declare global {
  interface Window {
    __omnideckMock: MockHandle;
  }
}

/**
 * Install the fake backend into `page`, before any app script runs. `overrides` is shallow-
 * merged over `defaultBackend()`, so a test that only wants the wizard passes a patched
 * `config` and inherits everything else.
 */
export async function installTauriMock(page: Page, overrides: Partial<Backend> = {}): Promise<void> {
  const backend: Backend = { ...defaultBackend(), ...overrides };

  await page.addInitScript((be: Backend) => {
    const callbacks = new Map<number, (e: unknown) => void>(); // transformCallback id -> fn
    const listeners = new Map<string, Map<number, number>>(); // event -> (eventId -> callback id)
    const unmocked: string[] = [];
    const calls: string[] = [];
    let nextCallback = 1;
    let nextEvent = 1;

    // Mutations are accepted and dropped: the frontend re-reads its own state after a save,
    // so persisting them would only let one test leak into the next.
    const accept = () => null;

    const handlers = {
      // --- boot ---
      get_capability: () => be.capability,
      get_config: () => be.config,
      get_library: () => be.library,
      get_apps: () => be.config.apps,
      get_catalog: () => be.catalog,
      in_gamescope_session: () => be.inSession,
      check_update: () => be.update,

      // --- art & icons (see the header note on omnideck:// and favicons) ---
      get_art: (a: { path: string }) => be.art[a.path] ?? null,
      bg_image: () => null,
      grid_art: () => null,
      app_icon: (a: { url: string }) => be.appIcons[a.url] ?? null,
      get_artwork: () => null,
      media_poster: () => null,

      // --- launching ---
      launch_game: accept,
      launch_command: accept,
      game_properties: accept,
      close_current_app: () => false,
      switch_app: () => false,
      power_action: accept,
      quit: accept,

      // --- persistence ---
      save_settings: accept,
      save_appearance: accept,
      save_apps: accept,
      save_favorites: accept,
      save_recent_apps: accept,
      backup_config: (a: { dest: string }) => a.dest,
      restore_config: () => be.config,

      // --- MPRIS / now playing ---
      media_now_playing: () => be.nowPlaying,
      media_control: accept,

      // --- media server ---
      media_available: () => be.mediaAvailable,
      media_sections: () => be.mediaSections,
      media_browse: (a: { parent: string }) => be.mediaBrowse[a.parent] ?? [],
      media_play: (a: { id: string }) => `${a.id}#1`,
      media_set_played: accept,

      // --- deck switcher ---
      deck_open: () => be.liveApps,
      deck_list: () => be.liveApps,
      deck_show: accept,
      deck_close: accept,
      deck_cancel: () => true,

      // --- sleep timer ---
      set_sleep_timer: (a: { minutes: number }) => ({ remaining_secs: a.minutes * 60, total_secs: a.minutes * 60 }),
      cancel_sleep_timer: () => false,
      get_sleep_timer: () => null,

      // --- the event plugin behind listen()/unlisten() ---
      "plugin:event|listen": (a: { event: string; handler: number }) => {
        const id = nextEvent++;
        let byId = listeners.get(a.event);
        if (!byId) listeners.set(a.event, (byId = new Map()));
        byId.set(id, a.handler);
        return id;
      },
      "plugin:event|unlisten": (a: { event: string; eventId: number }) => {
        listeners.get(a.event)?.delete(a.eventId);
        return null;
      },
      "plugin:event|emit": accept,
      "plugin:event|emit_to": accept,
    };
    // One lookup type for a table whose entries each want their own args shape.
    const table = handlers as unknown as Record<string, ((args: Record<string, unknown>) => unknown) | undefined>;

    // `@tauri-apps/api` declares these globals itself; alias rather than augment `Window`, so
    // the harness doesn't have to agree with its private shapes.
    const w = window as unknown as {
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
      __omnideckMock: MockHandle;
    };

    w.__TAURI_INTERNALS__ = {
      transformCallback(cb: (e: unknown) => void) {
        const id = nextCallback++;
        callbacks.set(id, cb);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
      convertFileSrc: (path: string) => path,
      invoke(cmd: string, args?: Record<string, unknown>) {
        calls.push(cmd);
        const handler = table[cmd];
        if (!handler) {
          // Loud, not silent: a new Tauri command that nobody taught the mock about shows up
          // in `window.__omnideckMock.unmocked` and fails the harness's own assertion.
          unmocked.push(cmd);
          return Promise.reject(new Error(`[tauri-mock] unmocked command: ${cmd}`));
        }
        try {
          return Promise.resolve(handler(args ?? {}));
        } catch (e) {
          return Promise.reject(e);
        }
      },
    };
    // `_unlisten()` calls this before its invoke; without it, component teardown throws.
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

    w.__omnideckMock = {
      unmocked,
      calls,
      emit(event: string, payload: unknown) {
        const byId = listeners.get(event);
        if (!byId) return 0;
        let delivered = 0;
        for (const [id, handlerId] of byId) {
          const cb = callbacks.get(handlerId);
          if (cb) {
            cb({ event, id, payload });
            delivered++;
          }
        }
        return delivered;
      },
    };
  }, backend);
}

/** Deliver a backend event to the running page (the mock's side of `tauri::Emitter`). */
export function emit(page: Page, event: string, payload: unknown = null): Promise<number> {
  return page.evaluate(([e, p]) => window.__omnideckMock.emit(e as string, p), [event, payload] as const);
}

/** Commands the app invoked that the mock doesn't implement — assert this stays empty. */
export function unmockedCommands(page: Page): Promise<string[]> {
  return page.evaluate(() => window.__omnideckMock.unmocked);
}
