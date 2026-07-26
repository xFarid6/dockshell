<script setup lang="ts">
import { ref } from "vue";
import type { ConnectionInfo } from "../api";

const emit = defineEmits<{
  save: [info: ConnectionInfo, secret: string | undefined];
}>();

const name = ref("");
const endpoint = ref("tcp://192.168.1.105:2375");
const useTls = ref(false);
const clientCertPath = ref("");
const caCertPath = ref("");
const keyPem = ref("");

function submit() {
  if (!name.value.trim() || !endpoint.value.trim()) return;
  emit(
    "save",
    {
      id: crypto.randomUUID(),
      name: name.value.trim(),
      endpoint: endpoint.value.trim(),
      useTls: useTls.value,
      clientCertPath: useTls.value ? clientCertPath.value.trim() || undefined : undefined,
      caCertPath: useTls.value ? caCertPath.value.trim() || undefined : undefined,
    },
    useTls.value ? keyPem.value || undefined : undefined,
  );
  name.value = "";
  clientCertPath.value = "";
  caCertPath.value = "";
  keyPem.value = "";
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
      TLS (client cert)
    </label>
    <template v-if="useTls">
      <input
        v-model="clientCertPath"
        placeholder="Client cert path (cert.pem)"
      >
      <input
        v-model="caCertPath"
        placeholder="CA cert path (ca.pem)"
      >
      <input
        v-model="keyPem"
        type="password"
        placeholder="Client key PEM (stored in OS keyring)"
      >
    </template>
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
