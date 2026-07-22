<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  containerAction,
  deleteConnection,
  listConnections,
  listContainers,
  saveConnection,
  testConnection,
  type ConnectionInfo,
  type ContainerAction,
  type ContainerInfo,
} from "./api";
import ConnectionForm from "./components/ConnectionForm.vue";
import ConnectionList from "./components/ConnectionList.vue";
import ContainerDetail from "./components/ContainerDetail.vue";
import ContainerTable from "./components/ContainerTable.vue";
import ExecTerminal from "./components/ExecTerminal.vue";
import TaskLogPanel from "./components/TaskLogPanel.vue";

const connections = ref<ConnectionInfo[]>([]);
const activeId = ref<string | null>(null);
const containers = ref<ContainerInfo[]>([]);
const busy = ref(false);
const error = ref("");
const taskLog = ref<string[]>([]);
const logsContainerId = ref<string | null>(null);
const execContainerId = ref<string | null>(null);
const detailContainerId = ref<string | null>(null);

function logTask(msg: string) {
  taskLog.value.unshift(`[${new Date().toLocaleTimeString()}] ${msg}`);
}

async function refreshConnections() {
  connections.value = await listConnections();
}

async function refreshContainers() {
  if (!activeId.value) return;
  busy.value = true;
  error.value = "";
  try {
    containers.value = await listContainers(activeId.value);
  } catch (e) {
    error.value = String(e);
    containers.value = [];
  } finally {
    busy.value = false;
  }
}

async function onSelect(id: string) {
  activeId.value = id;
  logsContainerId.value = null;
  execContainerId.value = null;
  detailContainerId.value = null;
  await refreshContainers();
}

function onLogs(containerId: string) {
  logsContainerId.value = logsContainerId.value === containerId ? null : containerId;
}

function onExec(containerId: string) {
  execContainerId.value = execContainerId.value === containerId ? null : containerId;
}

function onDetail(containerId: string) {
  detailContainerId.value = detailContainerId.value === containerId ? null : containerId;
}

async function onSave(info: ConnectionInfo, secret: string | undefined) {
  await saveConnection(info, secret);
  await refreshConnections();
  logTask(`saved connection "${info.name}"`);
}

async function onRemove(id: string) {
  await deleteConnection(id);
  if (activeId.value === id) {
    activeId.value = null;
    containers.value = [];
    logsContainerId.value = null;
    execContainerId.value = null;
  }
  await refreshConnections();
}

async function onTest() {
  if (!activeId.value) return;
  try {
    logTask(`test connection — ${await testConnection(activeId.value)}`);
  } catch (e) {
    logTask(`test connection failed — ${e}`);
  }
}

async function onAction(containerId: string, action: ContainerAction) {
  if (!activeId.value) return;
  busy.value = true;
  try {
    await containerAction(activeId.value, containerId, action);
    logTask(`${action} ${containerId.slice(0, 12)} — ok`);
  } catch (e) {
    logTask(`${action} ${containerId.slice(0, 12)} — failed: ${e}`);
  } finally {
    busy.value = false;
  }
  await refreshContainers();
}

onMounted(refreshConnections);
</script>

<template>
  <div class="layout">
    <aside class="sidebar">
      <h1>dockshell</h1>
      <ConnectionList
        :connections="connections"
        :active-id="activeId"
        @select="onSelect"
        @remove="onRemove"
      />
      <ConnectionForm @save="onSave" />
    </aside>
    <main class="main">
      <div class="toolbar">
        <button
          :disabled="!activeId || busy"
          @click="refreshContainers"
        >
          Refresh
        </button>
        <button
          :disabled="!activeId"
          @click="onTest"
        >
          Test connection
        </button>
        <span
          v-if="error"
          class="error"
        >{{ error }}</span>
      </div>
      <ContainerTable
        v-if="activeId"
        :containers="containers"
        :busy="busy"
        @action="onAction"
        @logs="onLogs"
        @exec="onExec"
        @detail="onDetail"
      />
      <p
        v-else
        class="hint"
      >
        Select or add a Docker host to get started.
      </p>
      <ContainerDetail
        v-if="activeId && detailContainerId"
        :connection-id="activeId"
        :container-id="detailContainerId"
        class="detail-panel"
        @close="detailContainerId = null"
      />
      <ExecTerminal
        v-if="activeId && execContainerId"
        :connection-id="activeId"
        :container-id="execContainerId"
        class="exec-panel"
      />
      <TaskLogPanel
        :entries="taskLog"
        :connection-id="activeId"
        :container-id="logsContainerId"
      />
    </main>
  </div>
</template>

<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 15px;
  color: #e8e8e8;
  background-color: #1d2126;
}
body {
  margin: 0;
}
button {
  border-radius: 6px;
  border: 1px solid transparent;
  padding: 0.4em 0.9em;
  font-family: inherit;
  color: inherit;
  background-color: #2c323a;
  cursor: pointer;
}
button:hover:not(:disabled) {
  border-color: #3f8cff;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
input {
  border-radius: 6px;
  border: 1px solid #3a414b;
  padding: 0.4em 0.6em;
  font-family: inherit;
  color: inherit;
  background-color: #262b32;
}
</style>

<style scoped>
.layout {
  display: flex;
  height: 100vh;
}
.sidebar {
  width: 260px;
  border-right: 1px solid rgba(128, 128, 128, 0.25);
  overflow-y: auto;
  padding: 0.5rem;
}
.sidebar h1 {
  font-size: 1.1rem;
  margin: 0.3rem 0.5rem 0.8rem;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}
.toolbar {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  padding: 0.5rem;
}
.error {
  color: #ff6b6b;
  font-size: 0.85rem;
}
.hint {
  padding: 1rem;
  opacity: 0.7;
}
.main > .container-table,
.main > table {
  flex: 1;
}
.exec-panel {
  flex: 1;
  min-height: 280px;
  border-top: 1px solid rgba(128, 128, 128, 0.25);
}
.detail-panel {
  max-height: 40vh;
}
.task-log-panel {
  margin-top: auto;
}
</style>
