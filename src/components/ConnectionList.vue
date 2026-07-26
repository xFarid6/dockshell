<script setup lang="ts">
import type { ConnectionInfo, HealthEvent } from "../api";

const props = defineProps<{
  connections: ConnectionInfo[];
  activeId: string | null;
  health?: Record<string, HealthEvent>;
}>();

defineEmits<{
  select: [id: string];
  remove: [id: string];
}>();

function healthTitle(id: string): string {
  const h = props.health?.[id];
  if (!h) return "Status unknown";
  if (h.status === "unreachable") return `Unreachable${h.error ? `: ${h.error}` : ""}`;
  if (h.status === "connecting") return "Connecting…";
  return "Connected";
}
</script>

<template>
  <ul class="connection-list">
    <li
      v-for="c in connections"
      :key="c.id"
      :class="{ active: c.id === activeId }"
    >
      <span
        class="health-dot"
        :class="health?.[c.id]?.status ?? 'unknown'"
        :title="healthTitle(c.id)"
      />
      <button
        class="name"
        @click="$emit('select', c.id)"
      >
        {{ c.name }}
        <span class="endpoint">{{ c.endpoint }}</span>
      </button>
      <button
        class="remove"
        title="Delete connection"
        @click="$emit('remove', c.id)"
      >
        ✕
      </button>
    </li>
    <li
      v-if="connections.length === 0"
      class="empty"
    >
      No connections yet.
    </li>
  </ul>
</template>

<style scoped>
.connection-list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.connection-list li {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}
.connection-list li.active .name {
  font-weight: 700;
}
.health-dot {
  width: 8px;
  height: 8px;
  min-width: 8px;
  border-radius: 50%;
  margin-left: 0.5rem;
  background-color: rgba(128, 128, 128, 0.5);
}
.health-dot.connected {
  background-color: #3ecf6b;
}
.health-dot.connecting {
  background-color: #e0a52c;
}
.health-dot.unreachable {
  background-color: #ff6b6b;
}
.name {
  flex: 1;
  text-align: left;
  background: none;
  border: none;
  box-shadow: none;
  cursor: pointer;
  padding: 0.4rem 0.5rem;
}
.endpoint {
  display: block;
  font-size: 0.75rem;
  opacity: 0.6;
}
.remove {
  background: none;
  border: none;
  box-shadow: none;
  cursor: pointer;
  opacity: 0.5;
}
.remove:hover {
  opacity: 1;
}
.empty {
  opacity: 0.6;
  padding: 0.4rem 0.5rem;
}
</style>
