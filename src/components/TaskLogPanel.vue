<script setup lang="ts">
// Task/log panel scaffold, same role as proxmox-desktop's task panel.
// Today it shows action results; live container-log streaming is issue #1.

defineProps<{
  entries: string[];
}>();
</script>

<template>
  <section class="task-log-panel">
    <header>Tasks</header>
    <ol v-if="entries.length > 0">
      <li
        v-for="(e, i) in entries"
        :key="i"
      >
        {{ e }}
      </li>
    </ol>
    <p
      v-else
      class="empty"
    >
      No tasks yet. Container actions get logged here.
    </p>
  </section>
</template>

<style scoped>
.task-log-panel {
  border-top: 1px solid rgba(128, 128, 128, 0.25);
  padding: 0.5rem;
  font-size: 0.85rem;
  max-height: 10rem;
  overflow-y: auto;
}
header {
  font-weight: 700;
  margin-bottom: 0.3rem;
}
ol {
  margin: 0;
  padding-left: 1.2rem;
}
.empty {
  opacity: 0.6;
  margin: 0;
}
</style>
