import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  containerAction,
  listConnections,
  listContainers,
  saveConnection,
  startLogStream,
  stopLogStream,
} from "../api";

describe("api wrappers", () => {
  beforeEach(() => invoke.mockReset());

  it("list_connections takes no args", async () => {
    invoke.mockResolvedValue([]);
    await listConnections();
    expect(invoke).toHaveBeenCalledWith("list_connections");
  });

  it("save_connection passes info and null secret by default", async () => {
    const info = { id: "1", name: "x", endpoint: "local", useTls: false };
    await saveConnection(info);
    expect(invoke).toHaveBeenCalledWith("save_connection", {
      info,
      secret: null,
    });
  });

  it("list_containers passes the connection id", async () => {
    invoke.mockResolvedValue([]);
    await listContainers("c1");
    expect(invoke).toHaveBeenCalledWith("list_containers", {
      connectionId: "c1",
    });
  });

  it("container_action passes ids and action", async () => {
    await containerAction("c1", "abc", "restart");
    expect(invoke).toHaveBeenCalledWith("container_action", {
      connectionId: "c1",
      containerId: "abc",
      action: "restart",
    });
  });

  it("start_log_stream passes connection and container ids", async () => {
    await startLogStream("c1", "abc");
    expect(invoke).toHaveBeenCalledWith("start_log_stream", {
      connectionId: "c1",
      containerId: "abc",
    });
  });

  it("stop_log_stream passes the container id", async () => {
    await stopLogStream("abc");
    expect(invoke).toHaveBeenCalledWith("stop_log_stream", {
      containerId: "abc",
    });
  });
});
