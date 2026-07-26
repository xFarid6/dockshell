<script setup lang="ts">
import { ref, watch } from "vue";
import { inspectContainer, type ContainerDetail } from "../api";

const props = defineProps<{
  connectionId: string;
  containerId: string;
}>();

defineEmits<{
  close: [];
}>();

const detail = ref<ContainerDetail | null>(null);
const error = ref("");
const copied = ref("");

async function load() {
  detail.value = null;
  error.value = "";
  try {
    detail.value = await inspectContainer(props.connectionId, props.containerId);
  } catch (e) {
    error.value = String(e);
  }
}

async function copy(value: string) {
  await navigator.clipboard.writeText(value);
  copied.value = value;
  setTimeout(() => {
    if (copied.value === value) copied.value = "";
  }, 1000);
}

watch(() => [props.connectionId, props.containerId], load, { immediate: true });
</script>

<template>
  <section class="container-detail">
    <header>
      <strong>{{ detail?.name ?? containerId.slice(0, 12) }}</strong>
      <button
        class="close"
        @click="$emit('close')"
      >
        Close
      </button>
    </header>

    <p
      v-if="error"
      class="error"
    >
      {{ error }}
    </p>

    <template v-else-if="detail">
      <dl class="summary">
        <dt>Image</dt>
        <dd>{{ detail.image }}</dd>
        <dt>State</dt>
        <dd>{{ detail.state }}<span v-if="detail.health"> ({{ detail.health }})</span></dd>
        <dt>Restart policy</dt>
        <dd>{{ detail.restartPolicy || "—" }}</dd>
        <dt>Created</dt>
        <dd>{{ detail.created || "—" }}</dd>
      </dl>

      <section class="group">
        <h3>Ports</h3>
        <ul v-if="detail.ports.length > 0">
          <li
            v-for="(p, i) in detail.ports"
            :key="i"
          >
            {{ p.hostIp }}:{{ p.hostPort }} → {{ p.containerPort }}
          </li>
        </ul>
        <p
          v-else
          class="empty"
        >
          No published ports.
        </p>
      </section>

      <section class="group">
        <h3>Mounts</h3>
        <ul v-if="detail.mounts.length > 0">
          <li
            v-for="(m, i) in detail.mounts"
            :key="i"
          >
            {{ m.source }} → {{ m.destination }} ({{ m.mode || "rw" }})
          </li>
        </ul>
        <p
          v-else
          class="empty"
        >
          No mounts.
        </p>
      </section>

      <section class="group">
        <h3>Env</h3>
        <ul v-if="detail.env.length > 0">
          <li
            v-for="e in detail.env"
            :key="e"
          >
            <button
              class="copy"
              @click="copy(e)"
            >
              {{ copied === e ? "Copied" : e }}
            </button>
          </li>
        </ul>
        <p
          v-else
          class="empty"
        >
          No environment variables.
        </p>
      </section>

      <section class="group">
        <h3>Labels</h3>
        <ul v-if="Object.keys(detail.labels).length > 0">
          <li
            v-for="(v, k) in detail.labels"
            :key="k"
          >
            <button
              class="copy"
              @click="copy(`${k}=${v}`)"
            >
              {{ copied === `${k}=${v}` ? "Copied" : `${k}=${v}` }}
            </button>
          </li>
        </ul>
        <p
          v-else
          class="empty"
        >
          No labels.
        </p>
      </section>
    </template>

    <p
      v-else
      class="empty"
    >
      Loading…
    </p>
  </section>
</template>

<style scoped>
.container-detail {
  border-top: 1px solid var(--color-border);
  padding: 0.6rem;
  font-size: 0.85rem;
  overflow-y: auto;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}
.close {
  padding: 0.1rem 0.5rem;
  font-size: 0.75rem;
}
.summary {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.1rem 0.6rem;
  margin: 0 0 0.6rem;
}
.summary dt {
  opacity: 0.6;
}
.summary dd {
  margin: 0;
}
.group {
  margin-bottom: 0.6rem;
}
.group h3 {
  font-size: 0.8rem;
  margin: 0 0 0.2rem;
  opacity: 0.75;
}
.group ul {
  list-style: none;
  margin: 0;
  padding: 0;
  font-family: monospace;
}
.copy {
  background: none;
  border: none;
  padding: 0.1rem 0;
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  text-align: left;
  cursor: pointer;
}
.copy:hover {
  color: var(--color-accent);
}
.empty {
  opacity: 0.6;
  margin: 0;
}
.error {
  color: var(--color-error);
}
</style>
