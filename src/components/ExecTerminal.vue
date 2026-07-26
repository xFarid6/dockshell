<script setup lang="ts">
// Docker-specific wiring around TerminalPane: turns exec-into-container into
// the pane's transport callbacks (write input, report resize) and forwards
// `exec-output:{execId}` events into it. All the xterm/Docker glue lives
// here so TerminalPane itself stays reusable.
import { onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  resizeExec,
  startExec,
  stopExec,
  writeExecInput,
  type ExecShell,
} from "../api";
import TerminalPane from "./TerminalPane.vue";

const props = defineProps<{
  connectionId: string;
  containerId: string;
}>();

const shell = ref<ExecShell>("/bin/sh");
const error = ref("");
const pane = ref<InstanceType<typeof TerminalPane> | null>(null);

let execId: string | null = null;
let unlisten: UnlistenFn | null = null;
let pendingResize: { cols: number; rows: number } | null = null;

// Bumped on every start() invocation. A superseded start() call (an
// out-of-order startExec()/listen() resolution left over from a prior
// container/connection/shell switch) re-checks this after each await and
// tears itself down instead of clobbering the current session (mirrors
// TaskLogPanel's #16 fix).
let generation = 0;

async function stop() {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  if (execId) {
    const id = execId;
    execId = null;
    await stopExec(id);
  }
}

async function start() {
  const gen = ++generation;
  await stop();
  error.value = "";
  try {
    const id = await startExec(props.connectionId, props.containerId, shell.value);
    if (gen !== generation) {
      await stopExec(id);
      return;
    }
    const stopListen = await listen<string>(`exec-output:${id}`, (event) => {
      if (gen !== generation) return;
      pane.value?.write(event.payload);
    });
    if (gen !== generation) {
      stopListen();
      await stopExec(id);
      return;
    }
    execId = id;
    unlisten = stopListen;
    if (pendingResize) {
      await resizeExec(id, pendingResize.cols, pendingResize.rows);
    }
  } catch (e) {
    error.value = String(e);
  }
}

function onInput(data: string) {
  if (execId) void writeExecInput(execId, data);
}

function onResize(cols: number, rows: number) {
  pendingResize = { cols, rows };
  if (execId) void resizeExec(execId, cols, rows);
}

watch(
  [shell, () => props.connectionId, () => props.containerId],
  start,
  { immediate: true },
);

onUnmounted(() => {
  generation++;
  void stop();
});
</script>

<template>
  <div class="exec-terminal">
    <div class="toolbar">
      <label>
        Shell
        <select v-model="shell">
          <option value="/bin/sh">
            /bin/sh
          </option>
          <option value="/bin/bash">
            /bin/bash
          </option>
        </select>
      </label>
      <span
        v-if="error"
        class="error"
      >{{ error }}</span>
    </div>
    <TerminalPane
      ref="pane"
      :on-input="onInput"
      :on-resize="onResize"
    />
  </div>
</template>

<style scoped>
.exec-terminal {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 280px;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.5rem;
  font-size: 0.85rem;
}
.error {
  color: var(--color-error);
}
</style>
