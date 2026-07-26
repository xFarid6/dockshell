<script setup lang="ts">
import type { NetworkInfo } from "../api";

defineProps<{
  networks: NetworkInfo[];
  busy: boolean;
}>();

defineEmits<{
  remove: [name: string];
}>();
</script>

<template>
  <div class="network-table">
    <table>
      <thead>
        <tr>
          <th>Name</th>
          <th>Driver</th>
          <th>Scope</th>
          <th>Subnet</th>
          <th>Attached containers</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="n in networks"
          :key="n.id"
        >
          <td>{{ n.name }}</td>
          <td>{{ n.driver }}</td>
          <td>{{ n.scope }}</td>
          <td>{{ n.subnet || "—" }}</td>
          <td>
            {{
              n.attachments.length > 0
                ? n.attachments.map((a) => a.container).join(", ")
                : "—"
            }}
          </td>
          <td class="actions">
            <button
              :disabled="busy || n.isBuiltin"
              :title="n.isBuiltin ? 'Built-in networks cannot be removed' : ''"
              @click="$emit('remove', n.name)"
            >
              Remove
            </button>
          </td>
        </tr>
        <tr v-if="networks.length === 0">
          <td
            colspan="6"
            class="empty"
          >
            No networks.
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.network-table table {
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
