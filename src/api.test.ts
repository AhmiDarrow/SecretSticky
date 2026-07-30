import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { api } from "./api";

describe("api invoke wrappers", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("status calls vault_status", async () => {
    invoke.mockResolvedValue({ initialized: true, unlocked: false });
    await api.status();
    expect(invoke).toHaveBeenCalledWith("vault_status");
  });

  it("setup passes password", async () => {
    invoke.mockResolvedValue("RECOVERY-KEY");
    await expect(api.setup("hunter22")).resolves.toBe("RECOVERY-KEY");
    expect(invoke).toHaveBeenCalledWith("vault_setup", { password: "hunter22" });
  });

  it("unlock / unlockRecovery / lock", async () => {
    invoke.mockResolvedValue(undefined);
    await api.unlock("pw");
    expect(invoke).toHaveBeenCalledWith("vault_unlock", { password: "pw" });
    await api.unlockRecovery("rk");
    expect(invoke).toHaveBeenCalledWith("vault_unlock_recovery", {
      recoveryKey: "rk",
    });
    await api.lock();
    expect(invoke).toHaveBeenCalledWith("vault_lock");
  });

  it("listNotes and getNote", async () => {
    invoke.mockResolvedValue([]);
    await api.listNotes();
    expect(invoke).toHaveBeenCalledWith("notes_list");
    invoke.mockResolvedValue({ id: "n1" });
    await api.getNote("n1");
    expect(invoke).toHaveBeenCalledWith("notes_get", { id: "n1" });
  });

  it("createNote omits color when undefined", async () => {
    invoke.mockResolvedValue({ id: "n" });
    await api.createNote();
    expect(invoke).toHaveBeenCalledWith("notes_create", { color: null });
    await api.createNote("blue");
    expect(invoke).toHaveBeenCalledWith("notes_create", { color: "blue" });
  });

  it("updateNote maps fields", async () => {
    invoke.mockResolvedValue({ id: "n1" });
    await api.updateNote({
      id: "n1",
      title: "t",
      body: "b",
      color: "pink",
      x: 1,
      y: 2,
      width: 300,
      height: 200,
      always_on_top: true,
    });
    expect(invoke).toHaveBeenCalledWith("notes_update", {
      id: "n1",
      title: "t",
      body: "b",
      color: "pink",
      x: 1,
      y: 2,
      width: 300,
      height: 200,
      always_on_top: true,
    });
  });

  it("delete / open / openAll / password / idle / window helpers", async () => {
    invoke.mockResolvedValue(undefined);
    await api.deleteNote("x");
    expect(invoke).toHaveBeenCalledWith("notes_delete", { id: "x" });
    await api.openNote("x");
    expect(invoke).toHaveBeenCalledWith("notes_open_window", { id: "x" });
    await api.openAll();
    expect(invoke).toHaveBeenCalledWith("notes_open_all");
    await api.changePassword("a", "b");
    expect(invoke).toHaveBeenCalledWith("change_password", {
      current: "a",
      newPassword: "b",
    });
    await api.setIdleLockSecs(900);
    expect(invoke).toHaveBeenCalledWith("set_idle_lock_secs", { secs: 900 });
    await api.touch();
    expect(invoke).toHaveBeenCalledWith("vault_touch");
    await api.checkIdle();
    expect(invoke).toHaveBeenCalledWith("vault_check_idle");
    await api.showMain();
    expect(invoke).toHaveBeenCalledWith("show_main");
    await api.hideMain();
    expect(invoke).toHaveBeenCalledWith("hide_main");
    await api.quitApp();
    expect(invoke).toHaveBeenCalledWith("quit_app");
    await api.openExternalUrl("https://github.com/AhmiDarrow");
    expect(invoke).toHaveBeenCalledWith("open_external_url", {
      url: "https://github.com/AhmiDarrow",
    });
  });
});
