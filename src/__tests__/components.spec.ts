import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

type EventHandler = (event: { payload: unknown }) => void;
const listen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listen(...args),
}));

import ConnectionList from "../components/ConnectionList.vue";
import ContainerTable from "../components/ContainerTable.vue";
import TaskLogPanel from "../components/TaskLogPanel.vue";
import type { ConnectionInfo, ContainerInfo } from "../api";

const conns: ConnectionInfo[] = [
  { id: "1", name: "wyse-server", endpoint: "tcp://192.168.1.105:2375", useTls: false },
  { id: "2", name: "local", endpoint: "local", useTls: false },
];

describe("ConnectionList", () => {
  it("renders the connection list", () => {
    const w = mount(ConnectionList, {
      props: { connections: conns, activeId: "1" },
    });
    expect(w.text()).toContain("wyse-server");
    expect(w.text()).toContain("tcp://192.168.1.105:2375");
    expect(w.findAll("li")).toHaveLength(2);
  });

  it("emits select on click", async () => {
    const w = mount(ConnectionList, {
      props: { connections: conns, activeId: null },
    });
    await w.find("button.name").trigger("click");
    expect(w.emitted("select")).toEqual([["1"]]);
  });

  it("shows an empty state", () => {
    const w = mount(ConnectionList, {
      props: { connections: [], activeId: null },
    });
    expect(w.text()).toContain("No connections yet");
  });
});

const containers: ContainerInfo[] = [
  {
    id: "abc123def456",
    name: "portainer",
    image: "portainer/portainer-ce:latest",
    state: "running",
    status: "Up 3 days",
    ports: ["9000:9000"],
  },
];

describe("ContainerTable", () => {
  it("renders container rows", () => {
    const w = mount(ContainerTable, {
      props: { containers, busy: false },
    });
    expect(w.text()).toContain("portainer");
    expect(w.text()).toContain("Up 3 days");
    expect(w.text()).toContain("9000:9000");
  });

  it("disables start for running containers and emits stop", async () => {
    const w = mount(ContainerTable, {
      props: { containers, busy: false },
    });
    const buttons = w.findAll("td.actions button");
    expect(buttons[0].attributes("disabled")).toBeDefined(); // start
    await buttons[1].trigger("click"); // stop
    expect(w.emitted("action")).toEqual([["abc123def456", "stop"]]);
  });

  it("emits logs when the Logs button is clicked", async () => {
    const w = mount(ContainerTable, {
      props: { containers, busy: false },
    });
    const buttons = w.findAll("td.actions button");
    await buttons[3].trigger("click"); // logs
    expect(w.emitted("logs")).toEqual([["abc123def456"]]);
  });

  it("emits exec when the Shell button is clicked on a running container", async () => {
    const w = mount(ContainerTable, {
      props: { containers, busy: false },
    });
    const buttons = w.findAll("td.actions button");
    expect(buttons[4].attributes("disabled")).toBeUndefined(); // running
    await buttons[4].trigger("click"); // shell
    expect(w.emitted("exec")).toEqual([["abc123def456"]]);
  });

  it("disables Shell for a stopped container", () => {
    const w = mount(ContainerTable, {
      props: {
        containers: [{ ...containers[0], state: "exited" }],
        busy: false,
      },
    });
    const buttons = w.findAll("td.actions button");
    expect(buttons[4].attributes("disabled")).toBeDefined();
  });
});

describe("TaskLogPanel", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
    listen.mockReset();
  });

  it("shows a placeholder when empty", () => {
    const w = mount(TaskLogPanel, { props: { entries: [] } });
    expect(w.text()).toContain("No tasks yet");
  });

  it("renders entries", () => {
    const w = mount(TaskLogPanel, {
      props: { entries: ["[12:00] start portainer — ok"] },
    });
    expect(w.text()).toContain("start portainer — ok");
  });

  it("starts a log stream and renders streamed lines", async () => {
    let handler: EventHandler | undefined;
    const unlisten = vi.fn();
    listen.mockImplementation((_event: string, h: EventHandler) => {
      handler = h;
      return Promise.resolve(unlisten);
    });

    const w = mount(TaskLogPanel, {
      props: { entries: [], connectionId: "c1", containerId: "abc123" },
    });
    await flushPromises();

    expect(listen).toHaveBeenCalledWith("log-line:abc123", expect.any(Function));
    expect(invoke).toHaveBeenCalledWith("start_log_stream", {
      connectionId: "c1",
      containerId: "abc123",
    });

    handler?.({ payload: { stream: "stdout", message: "hello" } });
    await flushPromises();
    expect(w.text()).toContain("hello");
  });

  it("stops the previous stream when containerId changes", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation(() => Promise.resolve(unlisten));

    const w = mount(TaskLogPanel, {
      props: { entries: [], connectionId: "c1", containerId: "abc123" },
    });
    await flushPromises();

    await w.setProps({ containerId: "def456" });
    await flushPromises();

    expect(unlisten).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("stop_log_stream", {
      containerId: "abc123",
    });
    expect(invoke).toHaveBeenCalledWith("start_log_stream", {
      connectionId: "c1",
      containerId: "def456",
    });
  });

  it("ignores a stale listen() resolution when switching containers faster than the IPC round-trip (#16)", async () => {
    const handlers = new Map<string, EventHandler>();
    const resolvers = new Map<string, () => void>();
    const unlistenFns = new Map<string, ReturnType<typeof vi.fn>>();

    listen.mockImplementation((event: string, handler: EventHandler) => {
      handlers.set(event, handler);
      const stop = vi.fn();
      unlistenFns.set(event, stop);
      return new Promise((resolve) => {
        resolvers.set(event, () => resolve(stop));
      });
    });

    const w = mount(TaskLogPanel, {
      props: { entries: [], connectionId: "c1", containerId: null },
    });
    await flushPromises();

    // Switch b -> c before b's listen() has resolved (faster than the IPC
    // round-trip), then let the resolutions arrive out of order: c first,
    // b's stale registration afterwards.
    await w.setProps({ containerId: "container-b" });
    await flushPromises();
    await w.setProps({ containerId: "container-c" });
    await flushPromises();

    resolvers.get("log-line:container-c")?.();
    await flushPromises();
    resolvers.get("log-line:container-b")?.();
    await flushPromises();

    // The stale (b) listener must tear itself down instead of being wired
    // up as "the" active unlisten, and must not tear down c's.
    expect(unlistenFns.get("log-line:container-b")).toHaveBeenCalledTimes(1);
    expect(unlistenFns.get("log-line:container-c")).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("stop_log_stream", {
      containerId: "container-b",
    });

    // Even if a log line arrives on the stale handler, it must be dropped
    // rather than appended to the current container's log.
    handlers.get("log-line:container-b")?.({
      payload: { stream: "stdout", message: "from-stale-b" },
    });
    handlers.get("log-line:container-c")?.({
      payload: { stream: "stdout", message: "from-current-c" },
    });
    await flushPromises();

    expect(w.text()).toContain("from-current-c");
    expect(w.text()).not.toContain("from-stale-b");
  });
});
