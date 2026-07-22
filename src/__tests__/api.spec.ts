import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  containerAction,
  createContainer,
  inspectContainer,
  listConnections,
  listContainers,
  listImages,
  pullImage,
  refreshHealthMonitors,
  removeImage,
  resizeExec,
  saveConnection,
  startEventStream,
  startExec,
  startLogStream,
  stopEventStream,
  stopExec,
  stopHealthMonitors,
  stopLogStream,
  writeExecInput,
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

  it("inspect_container passes the connection and container ids", async () => {
    await inspectContainer("c1", "abc");
    expect(invoke).toHaveBeenCalledWith("inspect_container", {
      connectionId: "c1",
      containerId: "abc",
    });
  });

  it("list_images passes the connection id", async () => {
    invoke.mockResolvedValue([]);
    await listImages("c1");
    expect(invoke).toHaveBeenCalledWith("list_images", { connectionId: "c1" });
  });

  it("remove_image passes connection id, image ref and force flag", async () => {
    await removeImage("c1", "alpine:latest", false);
    expect(invoke).toHaveBeenCalledWith("remove_image", {
      connectionId: "c1",
      image: "alpine:latest",
      force: false,
    });
  });

  it("pull_image passes the connection id and image ref", async () => {
    await pullImage("c1", "alpine:latest");
    expect(invoke).toHaveBeenCalledWith("pull_image", {
      connectionId: "c1",
      image: "alpine:latest",
    });
  });

  it("create_container passes image, name, ports and env", async () => {
    invoke.mockResolvedValue("newid");
    const ports = [{ host: "8080", container: "80" }];
    await createContainer("c1", "nginx:latest", "web", ports, ["FOO=bar"]);
    expect(invoke).toHaveBeenCalledWith("create_container", {
      connectionId: "c1",
      image: "nginx:latest",
      name: "web",
      ports,
      env: ["FOO=bar"],
    });
  });

  it("create_container defaults an omitted name to null", async () => {
    invoke.mockResolvedValue("newid");
    await createContainer("c1", "nginx:latest", undefined, [], []);
    expect(invoke).toHaveBeenCalledWith("create_container", {
      connectionId: "c1",
      image: "nginx:latest",
      name: null,
      ports: [],
      env: [],
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

  it("start_event_stream passes the connection id", async () => {
    await startEventStream("c1");
    expect(invoke).toHaveBeenCalledWith("start_event_stream", { connectionId: "c1" });
  });

  it("stop_event_stream passes the connection id", async () => {
    await stopEventStream("c1");
    expect(invoke).toHaveBeenCalledWith("stop_event_stream", { connectionId: "c1" });
  });

  it("start_exec passes connection, container and shell", async () => {
    invoke.mockResolvedValue("exec1");
    await startExec("c1", "abc", "/bin/bash");
    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c1",
      containerId: "abc",
      shell: "/bin/bash",
    });
  });

  it("write_exec_input passes the exec id and data", async () => {
    await writeExecInput("exec1", "ls\r");
    expect(invoke).toHaveBeenCalledWith("write_exec_input", {
      execId: "exec1",
      data: "ls\r",
    });
  });

  it("resize_exec passes the exec id and dimensions", async () => {
    await resizeExec("exec1", 80, 24);
    expect(invoke).toHaveBeenCalledWith("resize_exec", {
      execId: "exec1",
      cols: 80,
      rows: 24,
    });
  });

  it("stop_exec passes the exec id", async () => {
    await stopExec("exec1");
    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec1" });
  });

  it("refresh_health_monitors takes no args", async () => {
    await refreshHealthMonitors();
    expect(invoke).toHaveBeenCalledWith("refresh_health_monitors");
  });

  it("stop_health_monitors takes no args", async () => {
    await stopHealthMonitors();
    expect(invoke).toHaveBeenCalledWith("stop_health_monitors");
  });
});
