<script setup lang="ts">
// Generic xterm.js pane. Carries no Docker (or any backend) knowledge — it
// only knows how to render a terminal and report what the user typed or how
// big it is, via plain callback props. That keeps it liftable into a shared
// component (dockshell issue #2 notes this is meant to end up shared with
// the hopline project) without dragging exec/Docker specifics along.
import { onMounted, onUnmounted, ref } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const props = defineProps<{
  /** Called with each keystroke/paste the user makes in the terminal. */
  onInput: (data: string) => void;
  /** Called on mount and whenever the pane is resized, in character cells. */
  onResize: (cols: number, rows: number) => void;
}>();

const container = ref<HTMLDivElement | null>(null);
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let resizeObserver: ResizeObserver | null = null;

/** Write output into the terminal — called by the parent via template ref. */
function write(data: string) {
  term?.write(data);
}

defineExpose({ write });

onMounted(() => {
  if (!container.value) return;

  term = new Terminal({ convertEol: true, cursorBlink: true });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(container.value);
  fitAddon.fit();
  props.onResize(term.cols, term.rows);

  term.onData(props.onInput);

  resizeObserver = new ResizeObserver(() => {
    fitAddon?.fit();
    if (term) props.onResize(term.cols, term.rows);
  });
  resizeObserver.observe(container.value);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
  term?.dispose();
});
</script>

<template>
  <div
    ref="container"
    class="terminal-pane"
  />
</template>

<style scoped>
.terminal-pane {
  width: 100%;
  height: 100%;
  min-height: 240px;
}
</style>
