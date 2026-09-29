import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

import * as api from "./backend";

describe("task overview IPC contract", () => {
  beforeEach(() => { invoke.mockReset(); listen.mockReset(); });

  it("taps switch Home / last app without a launch id", async () => {
    await api.switchApp();
    expect(invoke).toHaveBeenCalledWith("switch_app", { id: null });
  });

  it("selects Home separately from cancel/restore", async () => {
    await api.deckHome();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("deck_home");
    await api.deckCancel();
    expect(invoke).toHaveBeenLastCalledWith("deck_cancel");
  });

  it("routes hold and tap as distinct events", async () => {
    const callback = vi.fn();
    await api.onGuideHold(callback);
    await api.onGuideTap(callback);
    expect(listen).toHaveBeenNthCalledWith(1, "guide-hold", callback);
    expect(listen).toHaveBeenNthCalledWith(2, "guide-tap", callback);
  });
});
