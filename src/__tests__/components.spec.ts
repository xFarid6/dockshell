import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

const open = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => open(...args),
}));

import ComposePanel from "../components/ComposePanel.vue";
import ConnectionForm from "../components/ConnectionForm.vue";
import ConnectionList from "../components/ConnectionList.vue";
import ContainerDetail from "../components/ContainerDetail.vue";
import ContainerTable from "../components/ContainerTable.vue";
import CreateContainerDialog from "../components/CreateContainerDialog.vue";
import ImageTable from "../components/ImageTable.vue";
import NetworkTable from "../components/NetworkTable.vue";
import SettingsDialog from "../components/SettingsDialog.vue";
import TaskLogPanel from "../components/TaskLogPanel.vue";
import VolumeTable from "../components/VolumeTable.vue";
import type {
  ConnectionInfo,
  ContainerDetail as ContainerDetailInfo,
  ContainerInfo,
  ImageInfo,
  NetworkInfo,
  Settings,
  VolumeInfo,
} from "../api";

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

describe("ConnectionForm", () => {
  it("hides TLS fields until the checkbox is ticked", async () => {
    const w = mount(ConnectionForm);
    expect(w.findAll("input").length).toBe(3); // name, endpoint, TLS checkbox
    await w.find("input[type='checkbox']").setValue(true);
    expect(w.findAll("input").length).toBe(6); // + cert path, CA path, key PEM
  });

  it("emits save with cert/CA paths and the key PEM as the secret when TLS is on", async () => {
    const w = mount(ConnectionForm);
    const inputs = w.findAll("input");
    await inputs[0].setValue("wyse-server");
    await inputs[1].setValue("tcp://192.168.1.105:2376");
    await w.find("input[type='checkbox']").setValue(true);
    const tlsInputs = w.findAll("input");
    await tlsInputs[3].setValue("/certs/cert.pem");
    await tlsInputs[4].setValue("/certs/ca.pem");
    await tlsInputs[5].setValue("-----BEGIN KEY-----");

    await w.find("form").trigger("submit");

    expect(w.emitted("save")).toHaveLength(1);
    const [info, secret] = w.emitted("save")![0] as [ConnectionInfo, string | undefined];
    expect(info.useTls).toBe(true);
    expect(info.clientCertPath).toBe("/certs/cert.pem");
    expect(info.caCertPath).toBe("/certs/ca.pem");
    expect(secret).toBe("-----BEGIN KEY-----");
  });

  it("omits cert paths and secret when TLS is off", async () => {
    const w = mount(ConnectionForm);
    const inputs = w.findAll("input");
    await inputs[0].setValue("local");
    await inputs[1].setValue("local");

    await w.find("form").trigger("submit");

    const [info, secret] = w.emitted("save")![0] as [ConnectionInfo, string | undefined];
    expect(info.useTls).toBe(false);
    expect(info.clientCertPath).toBeUndefined();
    expect(info.caCertPath).toBeUndefined();
    expect(secret).toBeUndefined();
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

  it("emits detail on row click but not on action-cell clicks", async () => {
    const w = mount(ContainerTable, {
      props: { containers, busy: false },
    });
    await w.find("td.actions button").trigger("click"); // start (disabled, but click still bubbles from td)
    expect(w.emitted("detail")).toBeUndefined();

    await w.find("tr[data-state]").trigger("click");
    expect(w.emitted("detail")).toEqual([["abc123def456"]]);
  });

  describe("live container events (#10)", () => {
    let handler: EventHandler | undefined;
    const unlistenFn = vi.fn();

    beforeEach(() => {
      invoke.mockReset();
      invoke.mockResolvedValue(undefined);
      listen.mockReset();
      unlistenFn.mockReset();
      listen.mockImplementation((_event: string, h: EventHandler) => {
        handler = h;
        return Promise.resolve(unlistenFn);
      });
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("starts an event stream and emits a debounced refresh on a container event", async () => {
      const w = mount(ContainerTable, {
        props: { containers, busy: false, connectionId: "c1" },
      });
      await flushPromises();

      expect(listen).toHaveBeenCalledWith("container-event:c1", expect.any(Function));
      expect(invoke).toHaveBeenCalledWith("start_event_stream", { connectionId: "c1" });

      handler?.({ payload: { action: "start", containerId: "abc", containerName: "web" } });
      await vi.advanceTimersByTimeAsync(300);

      expect(w.emitted("refresh")).toHaveLength(1);
    });

    it("coalesces a burst of events into a single refresh", async () => {
      const w = mount(ContainerTable, {
        props: { containers, busy: false, connectionId: "c1" },
      });
      await flushPromises();

      for (let i = 0; i < 5; i++) {
        handler?.({ payload: { action: "start", containerId: `c${i}`, containerName: null } });
        await vi.advanceTimersByTimeAsync(50);
      }
      await vi.advanceTimersByTimeAsync(300);

      expect(w.emitted("refresh")).toHaveLength(1);
    });

    it("stops the previous event stream when connectionId changes", async () => {
      const w = mount(ContainerTable, {
        props: { containers, busy: false, connectionId: "c1" },
      });
      await flushPromises();

      await w.setProps({ connectionId: "c2" });
      await flushPromises();

      expect(unlistenFn).toHaveBeenCalled();
      expect(invoke).toHaveBeenCalledWith("stop_event_stream", { connectionId: "c1" });
      expect(invoke).toHaveBeenCalledWith("start_event_stream", { connectionId: "c2" });
    });
  });
});

describe("ImageTable", () => {
  const images: ImageInfo[] = [
    { id: "sha256:abc123def456", tags: ["alpine:latest"], size: 7500000, created: 1751328000 },
  ];

  it("renders image rows", () => {
    const w = mount(ImageTable, { props: { images, busy: false } });
    expect(w.text()).toContain("alpine:latest");
    expect(w.text()).toContain("7.2 MB");
  });

  it("shows an empty state", () => {
    const w = mount(ImageTable, { props: { images: [], busy: false } });
    expect(w.text()).toContain("No images.");
  });

  it("emits pull with the trimmed input and clears it", async () => {
    const w = mount(ImageTable, { props: { images: [], busy: false } });
    const input = w.find("input");
    await input.setValue("  nginx:latest  ");
    await w.find(".pull-form button").trigger("click");
    expect(w.emitted("pull")).toEqual([["nginx:latest"]]);
    expect((input.element as HTMLInputElement).value).toBe("");
  });

  it("does not emit pull for a blank input", async () => {
    const w = mount(ImageTable, { props: { images: [], busy: false } });
    await w.find(".pull-form button").trigger("click");
    expect(w.emitted("pull")).toBeUndefined();
  });

  it("emits remove with the image's first tag", async () => {
    const w = mount(ImageTable, { props: { images, busy: false } });
    await w.findAll("td.actions button")[1].trigger("click");
    expect(w.emitted("remove")).toEqual([["alpine:latest"]]);
  });

  it("emits run with the image's first tag", async () => {
    const w = mount(ImageTable, { props: { images, busy: false } });
    await w.findAll("td.actions button")[0].trigger("click");
    expect(w.emitted("run")).toEqual([["alpine:latest"]]);
  });
});

describe("NetworkTable (#5)", () => {
  const networks: NetworkInfo[] = [
    {
      id: "n1",
      name: "app-net",
      driver: "bridge",
      scope: "local",
      subnet: "172.20.0.0/16",
      isBuiltin: false,
      attachments: [{ container: "portainer", ip: "172.20.0.2/16" }],
    },
    {
      id: "b1",
      name: "bridge",
      driver: "bridge",
      scope: "local",
      subnet: "172.17.0.0/16",
      isBuiltin: true,
      attachments: [],
    },
  ];

  it("renders network rows with their attached containers", () => {
    const w = mount(NetworkTable, { props: { networks, busy: false } });
    expect(w.text()).toContain("app-net");
    expect(w.text()).toContain("172.20.0.0/16");
    expect(w.text()).toContain("portainer");
  });

  it("shows a dash when a network has no attachments or subnet", () => {
    const w = mount(NetworkTable, {
      props: { networks: [networks[1]], busy: false },
    });
    expect(w.text()).toContain("—");
  });

  it("shows an empty state", () => {
    const w = mount(NetworkTable, { props: { networks: [], busy: false } });
    expect(w.text()).toContain("No networks.");
  });

  it("disables remove for a built-in network", () => {
    const w = mount(NetworkTable, { props: { networks, busy: false } });
    const buttons = w.findAll("td.actions button");
    expect(buttons[0].attributes("disabled")).toBeUndefined(); // app-net
    expect(buttons[1].attributes("disabled")).toBeDefined(); // bridge
  });

  it("emits remove with the network name", async () => {
    const w = mount(NetworkTable, { props: { networks, busy: false } });
    await w.findAll("td.actions button")[0].trigger("click");
    expect(w.emitted("remove")).toEqual([["app-net"]]);
  });
});

describe("VolumeTable", () => {
  const volumes: VolumeInfo[] = [
    {
      name: "data",
      driver: "local",
      mountpoint: "/var/lib/docker/volumes/data/_data",
      created: "2026-07-01T12:00:00Z",
      labels: {},
      usedBy: ["portainer"],
    },
  ];

  it("renders volume rows with their used-by containers", () => {
    const w = mount(VolumeTable, { props: { volumes, busy: false } });
    expect(w.text()).toContain("data");
    expect(w.text()).toContain("portainer");
  });

  it("shows a dash when a volume is unused", () => {
    const unused = [{ ...volumes[0], usedBy: [] }];
    const w = mount(VolumeTable, { props: { volumes: unused, busy: false } });
    expect(w.text()).toContain("—");
  });

  it("shows an empty state", () => {
    const w = mount(VolumeTable, { props: { volumes: [], busy: false } });
    expect(w.text()).toContain("No volumes.");
  });

  it("emits remove with the volume name", async () => {
    const w = mount(VolumeTable, { props: { volumes, busy: false } });
    await w.find(".actions button").trigger("click");
    expect(w.emitted("remove")).toEqual([["data"]]);
  });
});

describe("CreateContainerDialog", () => {
  it("emits create with trimmed name and filtered ports/env", async () => {
    const w = mount(CreateContainerDialog, {
      props: { image: "nginx:latest", busy: false },
    });

    await w.find("input[placeholder='my-container']").setValue("  web  ");
    const portInputs = w.findAll(".field-group")[0].findAll("input");
    await portInputs[0].setValue("8080");
    await portInputs[1].setValue("80");
    const envInput = w.findAll(".field-group")[1].find("input");
    await envInput.setValue("FOO=bar");

    await w.find(".actions button").trigger("click");

    expect(w.emitted("create")).toEqual([
      ["web", [{ host: "8080", container: "80" }], ["FOO=bar"]],
    ]);
  });

  it("drops blank port rows and blank env entries", async () => {
    const w = mount(CreateContainerDialog, {
      props: { image: "nginx:latest", busy: false },
    });

    await w.find(".actions button").trigger("click");

    expect(w.emitted("create")).toEqual([[undefined, [], []]]);
  });

  it("emits close on cancel", async () => {
    const w = mount(CreateContainerDialog, {
      props: { image: "nginx:latest", busy: false },
    });
    await w.findAll(".actions button")[1].trigger("click");
    expect(w.emitted("close")).toHaveLength(1);
  });
});

describe("SettingsDialog", () => {
  const settings: Settings = { theme: "system", refreshIntervalSecs: 10 };

  it("emits save with the edited theme and interval", async () => {
    const w = mount(SettingsDialog, { props: { settings } });
    await w.find("select").setValue("light");
    await w.find("input[type='number']").setValue(30);
    await w.find(".actions button").trigger("click");
    expect(w.emitted("save")).toEqual([[{ theme: "light", refreshIntervalSecs: 30 }]]);
  });

  it("clamps a zero/blank interval to 1 on save", async () => {
    const w = mount(SettingsDialog, { props: { settings } });
    await w.find("input[type='number']").setValue(0);
    await w.find(".actions button").trigger("click");
    expect(w.emitted("save")).toEqual([[{ theme: "system", refreshIntervalSecs: 1 }]]);
  });

  it("emits close on cancel", async () => {
    const w = mount(SettingsDialog, { props: { settings } });
    await w.findAll(".actions button")[1].trigger("click");
    expect(w.emitted("close")).toHaveLength(1);
  });
});

describe("ContainerDetail", () => {
  const detail: ContainerDetailInfo = {
    id: "abc123def456",
    name: "portainer",
    image: "portainer/portainer-ce:latest",
    state: "running",
    health: "healthy",
    created: "2026-07-01T00:00:00Z",
    restartPolicy: "unless-stopped",
    env: ["FOO=bar"],
    labels: { "com.example": "1" },
    mounts: [{ source: "/data", destination: "/var/lib/portainer", mode: "rw" }],
    ports: [{ containerPort: "9000/tcp", hostIp: "0.0.0.0", hostPort: "9000" }],
  };

  beforeEach(() => {
    invoke.mockReset();
  });

  it("loads and renders inspect data", async () => {
    invoke.mockResolvedValue(detail);
    const w = mount(ContainerDetail, {
      props: { connectionId: "c1", containerId: "abc123def456" },
    });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("inspect_container", {
      connectionId: "c1",
      containerId: "abc123def456",
    });
    expect(w.text()).toContain("portainer/portainer-ce:latest");
    expect(w.text()).toContain("running");
    expect(w.text()).toContain("healthy");
    expect(w.text()).toContain("unless-stopped");
    expect(w.text()).toContain("0.0.0.0:9000 → 9000/tcp");
    expect(w.text()).toContain("/data → /var/lib/portainer");
    expect(w.text()).toContain("FOO=bar");
    expect(w.text()).toContain("com.example=1");
  });

  it("shows the invoke error instead of the panel on failure", async () => {
    invoke.mockRejectedValue(new Error("no such container"));
    const w = mount(ContainerDetail, {
      props: { connectionId: "c1", containerId: "gone" },
    });
    await flushPromises();

    expect(w.text()).toContain("no such container");
  });

  it("re-fetches when the container id changes", async () => {
    invoke.mockResolvedValue(detail);
    const w = mount(ContainerDetail, {
      props: { connectionId: "c1", containerId: "abc123def456" },
    });
    await flushPromises();

    await w.setProps({ containerId: "other" });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith("inspect_container", {
      connectionId: "c1",
      containerId: "other",
    });
  });

  it("emits close", async () => {
    invoke.mockResolvedValue(detail);
    const w = mount(ContainerDetail, {
      props: { connectionId: "c1", containerId: "abc123def456" },
    });
    await flushPromises();

    await w.find("button.close").trigger("click");
    expect(w.emitted("close")).toHaveLength(1);
  });
});

describe("ComposePanel (#6)", () => {
  beforeEach(() => {
    open.mockReset();
  });

  it("shows a notice and disables inputs for a non-local connection", () => {
    const w = mount(ComposePanel, { props: { isLocal: false, busy: false } });
    expect(w.text()).toContain("Compose requires a local connection");
    expect(w.find("input").attributes("disabled")).toBeDefined();
  });

  it("does not emit up/down for a blank file path", async () => {
    const w = mount(ComposePanel, { props: { isLocal: true, busy: false } });
    const buttons = w.findAll("button");
    await buttons[1].trigger("click"); // Up
    await buttons[2].trigger("click"); // Down
    expect(w.emitted("up")).toBeUndefined();
    expect(w.emitted("down")).toBeUndefined();
  });

  it("emits up/down with the trimmed file path", async () => {
    const w = mount(ComposePanel, { props: { isLocal: true, busy: false } });
    await w.find("input").setValue("  /stacks/app.yml  ");
    const buttons = w.findAll("button");
    await buttons[1].trigger("click"); // Up
    await buttons[2].trigger("click"); // Down
    expect(w.emitted("up")).toEqual([["/stacks/app.yml"]]);
    expect(w.emitted("down")).toEqual([["/stacks/app.yml"]]);
  });

  it("does not emit while busy, even with a file path set", async () => {
    const w = mount(ComposePanel, { props: { isLocal: true, busy: true } });
    await w.find("input").setValue("/stacks/app.yml");
    await w.findAll("button")[1].trigger("click"); // Up
    expect(w.emitted("up")).toBeUndefined();
  });

  it("fills the file path from the browse dialog", async () => {
    open.mockResolvedValue("/picked/compose.yml");
    const w = mount(ComposePanel, { props: { isLocal: true, busy: false } });
    await w.findAll("button")[0].trigger("click"); // Browse…
    await flushPromises();
    expect((w.find("input").element as HTMLInputElement).value).toBe("/picked/compose.yml");
  });

  it("ignores a cancelled browse dialog", async () => {
    open.mockResolvedValue(null);
    const w = mount(ComposePanel, { props: { isLocal: true, busy: false } });
    await w.findAll("button")[0].trigger("click"); // Browse…
    await flushPromises();
    expect((w.find("input").element as HTMLInputElement).value).toBe("");
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
