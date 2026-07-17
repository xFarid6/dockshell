import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, type PropType } from "vue";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

type EventHandler = (event: { payload: unknown }) => void;
const listen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listen(...args),
}));

import ExecTerminal from "../components/ExecTerminal.vue";

// Stands in for the real xterm-backed TerminalPane: exposes the same
// onInput/onResize props and a write() method, without touching the DOM
// APIs xterm.js needs (canvas, ResizeObserver) that happy-dom doesn't fully
// provide.
const TerminalPaneStub = defineComponent({
  name: "TerminalPane",
  props: {
    onInput: { type: Function as PropType<(data: string) => void>, required: true },
    onResize: { type: Function as PropType<(cols: number, rows: number) => void>, required: true },
  },
  data() {
    return { written: [] as string[] };
  },
  methods: {
    write(data: string) {
      this.written.push(data);
    },
  },
  template: "<div></div>",
});

function mountExecTerminal() {
  return mount(ExecTerminal, {
    props: { connectionId: "c1", containerId: "abc123" },
    global: { stubs: { TerminalPane: TerminalPaneStub } },
  });
}

describe("ExecTerminal", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) =>
      cmd === "start_exec" ? Promise.resolve("exec1") : Promise.resolve(undefined),
    );
    listen.mockReset();
    listen.mockImplementation(() => Promise.resolve(vi.fn()));
  });

  it("starts an exec session for the default shell and subscribes to its output", async () => {
    mountExecTerminal();
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c1",
      containerId: "abc123",
      shell: "/bin/sh",
    });
    expect(listen).toHaveBeenCalledWith("exec-output:exec1", expect.any(Function));
  });

  it("writes incoming output into the terminal pane", async () => {
    let handler: EventHandler | undefined;
    listen.mockImplementation((_event: string, h: EventHandler) => {
      handler = h;
      return Promise.resolve(vi.fn());
    });

    const w = mountExecTerminal();
    await flushPromises();

    handler?.({ payload: "/ # " });
    await flushPromises();

    const pane = w.findComponent(TerminalPaneStub);
    expect(pane.vm.written).toEqual(["/ # "]);
  });

  it("forwards keystrokes to write_exec_input", async () => {
    const w = mountExecTerminal();
    await flushPromises();

    const pane = w.findComponent(TerminalPaneStub);
    pane.props().onInput("ls\r");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("write_exec_input", {
      execId: "exec1",
      data: "ls\r",
    });
  });

  it("forwards pane resizes to resize_exec", async () => {
    const w = mountExecTerminal();
    await flushPromises();

    const pane = w.findComponent(TerminalPaneStub);
    pane.props().onResize(80, 24);
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("resize_exec", {
      execId: "exec1",
      cols: 80,
      rows: 24,
    });
  });

  it("stops the previous session and starts a new one when the shell changes", async () => {
    const w = mountExecTerminal();
    await flushPromises();

    invoke.mockImplementation((cmd: string) =>
      cmd === "start_exec" ? Promise.resolve("exec2") : Promise.resolve(undefined),
    );

    await w.find("select").setValue("/bin/bash");
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec1" });
    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c1",
      containerId: "abc123",
      shell: "/bin/bash",
    });
  });

  it("stops the session on unmount", async () => {
    const w = mountExecTerminal();
    await flushPromises();

    invoke.mockClear();
    w.unmount();
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec1" });
  });

  it("stops the old session and starts a new one when containerId changes (#17)", async () => {
    invoke.mockImplementation((cmd: string, args?: { containerId?: string }) =>
      cmd === "start_exec"
        ? Promise.resolve(`exec-${args?.containerId}`)
        : Promise.resolve(undefined),
    );

    const w = mountExecTerminal();
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c1",
      containerId: "abc123",
      shell: "/bin/sh",
    });

    await w.setProps({ containerId: "def456" });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec-abc123" });
    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c1",
      containerId: "def456",
      shell: "/bin/sh",
    });
  });

  it("restarts the session when connectionId changes", async () => {
    invoke.mockImplementation((cmd: string, args?: { connectionId?: string }) =>
      cmd === "start_exec"
        ? Promise.resolve(`exec-${args?.connectionId}`)
        : Promise.resolve(undefined),
    );

    const w = mountExecTerminal();
    await flushPromises();

    await w.setProps({ connectionId: "c2" });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec-c1" });
    expect(invoke).toHaveBeenCalledWith("start_exec", {
      connectionId: "c2",
      containerId: "abc123",
      shell: "/bin/sh",
    });
  });

  it("ignores a stale startExec/listen() resolution when switching containers faster than the IPC round-trip (#17)", async () => {
    const startExecResolvers = new Map<string, () => void>();
    invoke.mockImplementation((cmd: string, args?: { containerId?: string }) => {
      if (cmd === "start_exec") {
        const containerId = args?.containerId ?? "";
        return new Promise<string>((resolve) => {
          startExecResolvers.set(containerId, () => resolve(`exec-${containerId}`));
        });
      }
      return Promise.resolve(undefined);
    });

    const handlers = new Map<string, EventHandler>();
    const listenResolvers = new Map<string, () => void>();
    const unlistenFns = new Map<string, ReturnType<typeof vi.fn>>();
    listen.mockImplementation((event: string, handler: EventHandler) => {
      handlers.set(event, handler);
      const stop = vi.fn();
      unlistenFns.set(event, stop);
      return new Promise((resolve) => {
        listenResolvers.set(event, () => resolve(stop));
      });
    });

    const w = mountExecTerminal(); // containerId: "abc123"
    await flushPromises();

    // Resolve the first container's start_exec but not (yet) its listen(),
    // then switch away before that listen() resolves — matching the shape
    // of the out-of-order race TaskLogPanel hit in #16.
    startExecResolvers.get("abc123")?.();
    await flushPromises();

    await w.setProps({ containerId: "container-c" });
    await flushPromises();

    startExecResolvers.get("container-c")?.();
    await flushPromises();

    // Resolve out of order: the new (c) session's listener first, then the
    // stale (abc123) one left over from before the switch.
    listenResolvers.get("exec-output:exec-container-c")?.();
    await flushPromises();
    listenResolvers.get("exec-output:exec-abc123")?.();
    await flushPromises();

    // The stale listener must tear itself (and its exec session) down
    // instead of being wired up as "the" active session, and must not touch
    // the current one.
    expect(unlistenFns.get("exec-output:exec-abc123")).toHaveBeenCalledTimes(1);
    expect(unlistenFns.get("exec-output:exec-container-c")).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("stop_exec", { execId: "exec-abc123" });
    expect(invoke).not.toHaveBeenCalledWith("stop_exec", { execId: "exec-container-c" });

    // Output arriving on the stale handler must be dropped; output on the
    // current handler must still reach the pane.
    handlers.get("exec-output:exec-abc123")?.({ payload: "stale" });
    handlers.get("exec-output:exec-container-c")?.({ payload: "current" });
    await flushPromises();

    const pane = w.findComponent(TerminalPaneStub);
    expect(pane.vm.written).toEqual(["current"]);
  });
});
