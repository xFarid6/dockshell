<script setup lang="ts">
import { ref } from "vue";
import type { ConnectionInfo } from "../api";

const emit = defineEmits<{
  save: [info: ConnectionInfo, secret: string | undefined];
}>();

const name = ref("");
const endpoint = ref("tcp://192.168.1.105:2375");
const useTls = ref(false);
const secret = ref("");

function submit() {
  if (!name.value.trim() || !endpoint.value.trim()) return;
  emit(
    "save",
    {
      id: crypto.randomUUID(),
      name: name.value.trim(),
      endpoint: endpoint.value.trim(),
      useTls: useTls.value,
    },
    secret.value || undefined,
  );
  name.value = "";
  secret.value = "";
}
</script>

<template>
  <form
    class="connection-form"
    @submit.prevent="submit"
  >
    <input
      v-model="name"
      placeholder="Name (e.g. wyse-server)"
    >
    <input
      v-model="endpoint"
      placeholder="&quot;local&quot; or tcp://host:2375"
    >
    <label>
      <input
        v-model="useTls"
        type="checkbox"
      >
      TLS (client cert — issue #7)
    </label>
    <input
      v-model="secret"
      type="password"
      placeholder="Secret (optional, stored in OS keyring)"
    >
    <button type="submit">
      Add connection
    </button>
  </form>
</template>

<style scoped>
.connection-form {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  padding: 0.5rem;
}
label {
  font-size: 0.85rem;
}
</style>
