<script setup lang="ts">
// Refreshes itself on container lifecycle events (start/stop/die/...) instead
// of relying solely on the manual Refresh button, debounced since one
// `docker compose up` emits many events in a burst.
import { onUnmounted, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  startEventStream,
  stopEventStream,
  type ContainerAction,
  type ContainerEvent,
  type ContainerInfo,
} from "../api";

const REFRESH_DEBOUNCE_MS = 300;

const props = defineProps<{
  containers: ContainerInfo[];
  busy: boolean;
  connectionId?: string | null;
}>();

const emit = defineEmits<{
  action: [containerId: string, action: ContainerAction];
  logs: [containerId: string];
  exec: [containerId: string];
  detail: [containerId: string];
  refresh: [];
}>();

let unlisten: UnlistenFn | null = null;
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

// Bumped on every connection switch. subscribe() re-checks this after each
// await so a listen()/startEventStream() call left over from a superseded
// switch tears itself down instead of clobbering the current one (mirrors
// TaskLogPanel's #16 fix).
let generation = 0;

function scheduleRefresh() {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => emit("refresh"), REFRESH_DEBOUNCE_MS);
}

async function subscribe(connectionId: string, gen: number) {
  const stop = await listen<ContainerEvent>(`container-event:${connectionId}`, () => {
    if (gen !== generation) return;
    scheduleRefresh();
  });
  if (gen !== generation) {
    stop();
    await stopEventStream(connectionId);
    return;
  }
  unlisten = stop;
  await startEventStream(connectionId);
}

async function unsubscribe(connectionId: string) {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  await stopEventStream(connectionId);
}

watch(
  () => props.connectionId,
  async (newId, oldId) => {
    const gen = ++generation;
    if (oldId) await unsubscribe(oldId);
    if (newId) await subscribe(newId, gen);
  },
  { immediate: true },
);

onUnmounted(() => {
  generation++;
  if (props.connectionId) unsubscribe(props.connectionId);
});
</script>

<template>
  <table class="container-table">
    <thead>
      <tr>
        <th>Name</th>
        <th>Image</th>
        <th>State</th>
        <th>Status</th>
        <th>Ports</th>
        <th>Actions</th>
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="c in containers"
        :key="c.id"
        :data-state="c.state"
        class="row"
        @click="$emit('detail', c.id)"
      >
        <td>{{ c.name }}</td>
        <td>{{ c.image }}</td>
        <td>{{ c.state }}</td>
        <td>{{ c.status }}</td>
        <td>{{ c.ports.join(", ") }}</td>
        <td
          class="actions"
          @click.stop
        >
          <button
            :disabled="busy || c.state === 'running'"
            @click="$emit('action', c.id, 'start')"
          >
            Start
          </button>
          <button
            :disabled="busy || c.state !== 'running'"
            @click="$emit('action', c.id, 'stop')"
          >
            Stop
          </button>
          <button
            :disabled="busy"
            @click="$emit('action', c.id, 'restart')"
          >
            Restart
          </button>
          <button @click="$emit('logs', c.id)">
            Logs
          </button>
          <button
            :disabled="c.state !== 'running'"
            @click="$emit('exec', c.id)"
          >
            Shell
          </button>
        </td>
      </tr>
      <tr v-if="containers.length === 0">
        <td
          colspan="6"
          class="empty"
        >
          No containers.
        </td>
      </tr>
    </tbody>
  </table>
</template>

<style scoped>
.container-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}
th,
td {
  text-align: left;
  padding: 0.4rem 0.6rem;
  border-bottom: 1px solid var(--color-border);
}
.actions button {
  margin-right: 0.3rem;
  padding: 0.2rem 0.6rem;
  font-size: 0.8rem;
}
.row {
  cursor: pointer;
}
.row:hover {
  background-color: var(--color-hover);
}
.empty {
  opacity: 0.6;
}
</style>
