<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  containerAction,
  createContainer,
  deleteConnection,
  listConnections,
  listContainers,
  listImages,
  pruneContainers,
  pruneImages,
  pruneNetworks,
  pruneVolumes,
  pullImage,
  removeImage,
  saveConnection,
  testConnection,
  type ConnectionInfo,
  type ContainerAction,
  type ContainerInfo,
  type ImageInfo,
  type PortMapping,
  type PruneResult,
  type PullProgress,
} from "./api";
import CleanupDialog from "./components/CleanupDialog.vue";
import ConnectionForm from "./components/ConnectionForm.vue";
import ConnectionList from "./components/ConnectionList.vue";
import ContainerDetail from "./components/ContainerDetail.vue";
import ContainerTable from "./components/ContainerTable.vue";
import CreateContainerDialog from "./components/CreateContainerDialog.vue";
import ExecTerminal from "./components/ExecTerminal.vue";
import ImageTable from "./components/ImageTable.vue";
import TaskLogPanel from "./components/TaskLogPanel.vue";

const connections = ref<ConnectionInfo[]>([]);
const activeId = ref<string | null>(null);
const view = ref<"containers" | "images">("containers");
const containers = ref<ContainerInfo[]>([]);
const images = ref<ImageInfo[]>([]);
const busy = ref(false);
const imagesBusy = ref(false);
const error = ref("");
const taskLog = ref<string[]>([]);
const logsContainerId = ref<string | null>(null);
const execContainerId = ref<string | null>(null);
const detailContainerId = ref<string | null>(null);
const runImage = ref<string | null>(null);
const cleanupOpen = ref(false);
const cleanupBusy = ref(false);

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

async function refreshImages() {
  if (!activeId.value) return;
  imagesBusy.value = true;
  error.value = "";
  try {
    images.value = await listImages(activeId.value);
  } catch (e) {
    error.value = String(e);
    images.value = [];
  } finally {
    imagesBusy.value = false;
  }
}

async function onSelect(id: string) {
  activeId.value = id;
  logsContainerId.value = null;
  execContainerId.value = null;
  detailContainerId.value = null;
  await refreshContainers();
  if (view.value === "images") await refreshImages();
}

async function onSwitchView(next: "containers" | "images") {
  view.value = next;
  if (next === "images") await refreshImages();
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

async function onPull(image: string) {
  if (!activeId.value) return;
  const connectionId = activeId.value;
  imagesBusy.value = true;
  logTask(`pull ${image} — starting`);
  const unlisten = await listen<PullProgress>(`pull-progress:${image}`, (event) => {
    const p = event.payload;
    logTask(`pull ${image} — ${p.status}${p.progress ? " " + p.progress : ""}`);
  });
  try {
    await pullImage(connectionId, image);
    logTask(`pull ${image} — done`);
  } catch (e) {
    logTask(`pull ${image} — failed: ${e}`);
  } finally {
    unlisten();
    imagesBusy.value = false;
    await refreshImages();
  }
}

async function onRunImage(name: string | undefined, ports: PortMapping[], env: string[]) {
  if (!activeId.value || !runImage.value) return;
  const image = runImage.value;
  imagesBusy.value = true;
  try {
    const id = await createContainer(activeId.value, image, name, ports, env);
    logTask(`run ${image} — ${id.slice(0, 12)}`);
    runImage.value = null;
  } catch (e) {
    logTask(`run ${image} — failed: ${e}`);
  } finally {
    imagesBusy.value = false;
  }
}

async function onRemoveImage(image: string) {
  if (!activeId.value) return;
  imagesBusy.value = true;
  try {
    await removeImage(activeId.value, image, false);
    logTask(`remove image ${image} — ok`);
  } catch (e) {
    logTask(`remove image ${image} — failed: ${e}`);
  } finally {
    imagesBusy.value = false;
  }
  await refreshImages();
}

function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
}

type PruneCategory = "containers" | "images" | "volumes" | "networks";

const PRUNE_FNS: Record<PruneCategory, (connectionId: string) => Promise<PruneResult>> = {
  containers: pruneContainers,
  images: pruneImages,
  volumes: pruneVolumes,
  networks: pruneNetworks,
};

async function onPrune(categories: PruneCategory[]) {
  if (!activeId.value) return;
  const connectionId = activeId.value;
  cleanupBusy.value = true;
  for (const category of categories) {
    try {
      const result = await PRUNE_FNS[category](connectionId);
      const freed =
        result.spaceReclaimed != null ? `, freed ${formatBytes(result.spaceReclaimed)}` : "";
      logTask(`prune ${category} — removed ${result.deleted.length}${freed}`);
    } catch (e) {
      logTask(`prune ${category} — failed: ${e}`);
    }
  }
  cleanupBusy.value = false;
  cleanupOpen.value = false;
  if (categories.includes("containers")) await refreshContainers();
  if (categories.includes("images")) await refreshImages();
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
        <nav class="view-nav">
          <button
            :class="{ active: view === 'containers' }"
            @click="onSwitchView('containers')"
          >
            Containers
          </button>
          <button
            :class="{ active: view === 'images' }"
            @click="onSwitchView('images')"
          >
            Images
          </button>
        </nav>
        <button
          v-if="view === 'containers'"
          :disabled="!activeId || busy"
          @click="refreshContainers"
        >
          Refresh
        </button>
        <button
          v-else
          :disabled="!activeId || imagesBusy"
          @click="refreshImages"
        >
          Refresh
        </button>
        <button
          :disabled="!activeId"
          @click="onTest"
        >
          Test connection
        </button>
        <button
          :disabled="!activeId"
          @click="cleanupOpen = true"
        >
          Clean up
        </button>
        <span
          v-if="error"
          class="error"
        >{{ error }}</span>
      </div>
      <ContainerTable
        v-if="activeId && view === 'containers'"
        :containers="containers"
        :busy="busy"
        :connection-id="activeId"
        @action="onAction"
        @logs="onLogs"
        @exec="onExec"
        @detail="onDetail"
        @refresh="refreshContainers"
      />
      <ImageTable
        v-else-if="activeId && view === 'images'"
        :images="images"
        :busy="imagesBusy"
        @pull="onPull"
        @remove="onRemoveImage"
        @run="runImage = $event"
      />
      <CreateContainerDialog
        v-if="activeId && view === 'images' && runImage"
        :image="runImage"
        :busy="imagesBusy"
        @create="onRunImage"
        @close="runImage = null"
      />
      <p
        v-else
        class="hint"
      >
        Select or add a Docker host to get started.
      </p>
      <ContainerDetail
        v-if="activeId && view === 'containers' && detailContainerId"
        :connection-id="activeId"
        :container-id="detailContainerId"
        class="detail-panel"
        @close="detailContainerId = null"
      />
      <ExecTerminal
        v-if="activeId && view === 'containers' && execContainerId"
        :connection-id="activeId"
        :container-id="execContainerId"
        class="exec-panel"
      />
      <CleanupDialog
        v-if="cleanupOpen"
        :busy="cleanupBusy"
        @prune="onPrune"
        @close="cleanupOpen = false"
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
.view-nav {
  display: flex;
  gap: 0.3rem;
  margin-right: 0.5rem;
}
.view-nav button.active {
  border-color: #3f8cff;
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
