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
  await stop();
  error.value = "";
  try {
    const id = await startExec(props.connectionId, props.containerId, shell.value);
    execId = id;
    unlisten = await listen<string>(`exec-output:${id}`, (event) => {
      pane.value?.write(event.payload);
    });
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

watch(shell, start, { immediate: true });

onUnmounted(stop);
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
  color: #ff6b6b;
}
</style>
