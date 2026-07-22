<script setup lang="ts">
import { ref } from "vue";
import type { PortMapping } from "../api";

const props = defineProps<{
  image: string;
  busy: boolean;
}>();

const emit = defineEmits<{
  create: [name: string | undefined, ports: PortMapping[], env: string[]];
  close: [];
}>();

const name = ref("");
const ports = ref<PortMapping[]>([{ host: "", container: "" }]);
const env = ref<string[]>([""]);

function submit() {
  const cleanPorts = ports.value.filter((p) => p.host.trim() && p.container.trim());
  const cleanEnv = env.value.map((e) => e.trim()).filter(Boolean);
  emit("create", name.value.trim() || undefined, cleanPorts, cleanEnv);
}
</script>

<template>
  <div class="dialog">
    <h3>Run {{ props.image }}</h3>
    <label>
      Name (optional)
      <input
        v-model="name"
        placeholder="my-container"
      >
    </label>

    <div class="field-group">
      <span class="label">Ports (host : container)</span>
      <div
        v-for="(p, i) in ports"
        :key="i"
        class="row"
      >
        <input
          v-model="p.host"
          placeholder="8080"
        >
        <span>:</span>
        <input
          v-model="p.container"
          placeholder="80"
        >
        <button
          type="button"
          @click="ports.splice(i, 1)"
        >
          &minus;
        </button>
      </div>
      <button
        type="button"
        @click="ports.push({ host: '', container: '' })"
      >
        + Port
      </button>
    </div>

    <div class="field-group">
      <span class="label">Environment (KEY=VALUE)</span>
      <div
        v-for="(_, i) in env"
        :key="i"
        class="row"
      >
        <input
          v-model="env[i]"
          placeholder="KEY=value"
        >
        <button
          type="button"
          @click="env.splice(i, 1)"
        >
          &minus;
        </button>
      </div>
      <button
        type="button"
        @click="env.push('')"
      >
        + Env
      </button>
    </div>

    <div class="actions">
      <button
        :disabled="busy"
        @click="submit"
      >
        Run
      </button>
      <button
        :disabled="busy"
        @click="$emit('close')"
      >
        Cancel
      </button>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 8px;
  padding: 0.8rem 1rem;
  margin: 0.5rem 0.6rem;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.dialog h3 {
  margin: 0;
}
label {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  font-size: 0.85rem;
}
.field-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
.label {
  font-size: 0.85rem;
  opacity: 0.8;
}
.row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.actions {
  display: flex;
  gap: 0.5rem;
}
</style>
