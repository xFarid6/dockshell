import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
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
});

describe("TaskLogPanel", () => {
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
});
