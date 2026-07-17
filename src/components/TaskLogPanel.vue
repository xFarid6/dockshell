<script setup lang="ts">
// Task/log panel: renders one-shot action results (`entries`) plus, when a
// container is selected for viewing, its live streamed logs.
import { onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { startLogStream, stopLogStream, type LogLine } from "../api";

const MAX_LOG_LINES = 5000;

const props = defineProps<{
  entries: string[];
  connectionId?: string | null;
  containerId?: string | null;
}>();

const logLines = ref<LogLine[]>([]);
let unlisten: UnlistenFn | null = null;

async function subscribe(connectionId: string, containerId: string) {
  logLines.value = [];
  unlisten = await listen<LogLine>(`log-line:${containerId}`, (event) => {
    logLines.value.push(event.payload);
    if (logLines.value.length > MAX_LOG_LINES) {
      logLines.value.splice(0, logLines.value.length - MAX_LOG_LINES);
    }
  });
  await startLogStream(connectionId, containerId);
}

async function unsubscribe(containerId: string) {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  await stopLogStream(containerId);
}

watch(
  () => props.containerId,
  async (newId, oldId) => {
    if (oldId) await unsubscribe(oldId);
    if (newId && props.connectionId) await subscribe(props.connectionId, newId);
  },
  { immediate: true },
);

onUnmounted(() => {
  if (props.containerId) unsubscribe(props.containerId);
});
</script>

<template>
  <section class="task-log-panel">
    <header>Tasks</header>
    <ol v-if="entries.length > 0">
      <li
        v-for="(e, i) in entries"
        :key="i"
      >
        {{ e }}
      </li>
    </ol>
    <p
      v-else
      class="empty"
    >
      No tasks yet. Container actions get logged here.
    </p>

    <template v-if="containerId">
      <header>Logs — {{ containerId.slice(0, 12) }}</header>
      <ol
        v-if="logLines.length > 0"
        class="log-lines"
      >
        <li
          v-for="(line, i) in logLines"
          :key="i"
          :class="{ stderr: line.stream === 'stderr' }"
        >
          {{ line.message }}
        </li>
      </ol>
      <p
        v-else
        class="empty"
      >
        Waiting for log output…
      </p>
    </template>
  </section>
</template>

<style scoped>
.task-log-panel {
  border-top: 1px solid rgba(128, 128, 128, 0.25);
  padding: 0.5rem;
  font-size: 0.85rem;
  max-height: 10rem;
  overflow-y: auto;
}
header {
  font-weight: 700;
  margin-bottom: 0.3rem;
}
ol {
  margin: 0;
  padding-left: 1.2rem;
}
.log-lines {
  list-style: none;
  padding-left: 0;
  font-family: monospace;
}
.log-lines li.stderr {
  color: #ff6b6b;
}
.empty {
  opacity: 0.6;
  margin: 0;
}
</style>
