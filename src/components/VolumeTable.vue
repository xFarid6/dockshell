<script setup lang="ts">
import type { VolumeInfo } from "../api";

defineProps<{
  volumes: VolumeInfo[];
  busy: boolean;
}>();

defineEmits<{
  remove: [name: string];
}>();
</script>

<template>
  <div class="volume-table">
    <table>
      <thead>
        <tr>
          <th>Name</th>
          <th>Driver</th>
          <th>Mountpoint</th>
          <th>Created</th>
          <th>Used by</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="v in volumes"
          :key="v.name"
        >
          <td>{{ v.name }}</td>
          <td>{{ v.driver }}</td>
          <td>{{ v.mountpoint }}</td>
          <td>{{ v.created }}</td>
          <td>{{ v.usedBy.length > 0 ? v.usedBy.join(", ") : "—" }}</td>
          <td class="actions">
            <button
              :disabled="busy"
              @click="$emit('remove', v.name)"
            >
              Remove
            </button>
          </td>
        </tr>
        <tr v-if="volumes.length === 0">
          <td
            colspan="6"
            class="empty"
          >
            No volumes.
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.volume-table table {
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
  padding: 0.2rem 0.6rem;
  font-size: 0.8rem;
}
.empty {
  opacity: 0.6;
}
</style>
