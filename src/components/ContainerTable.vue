<script setup lang="ts">
import type { ContainerAction, ContainerInfo } from "../api";

defineProps<{
  containers: ContainerInfo[];
  busy: boolean;
}>();

defineEmits<{
  action: [containerId: string, action: ContainerAction];
}>();
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
      >
        <td>{{ c.name }}</td>
        <td>{{ c.image }}</td>
        <td>{{ c.state }}</td>
        <td>{{ c.status }}</td>
        <td>{{ c.ports.join(", ") }}</td>
        <td class="actions">
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
  border-bottom: 1px solid rgba(128, 128, 128, 0.25);
}
.actions button {
  margin-right: 0.3rem;
  padding: 0.2rem 0.6rem;
  font-size: 0.8rem;
}
.empty {
  opacity: 0.6;
}
</style>
