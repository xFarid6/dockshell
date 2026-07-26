<script setup lang="ts">
import { ref } from "vue";
import type { ImageInfo } from "../api";

defineProps<{
  images: ImageInfo[];
  busy: boolean;
}>();

const emit = defineEmits<{
  pull: [image: string];
  remove: [image: string];
  run: [image: string];
}>();

const pullRef = ref("");

function submitPull() {
  const image = pullRef.value.trim();
  if (!image) return;
  emit("pull", image);
  pullRef.value = "";
}

function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
}

function formatCreated(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}
</script>

<template>
  <div class="image-table">
    <div class="pull-form">
      <input
        v-model="pullRef"
        placeholder="image to pull, e.g. alpine:latest"
        @keyup.enter="submitPull"
      >
      <button
        :disabled="busy || !pullRef.trim()"
        @click="submitPull"
      >
        Pull
      </button>
    </div>
    <table>
      <thead>
        <tr>
          <th>Tags</th>
          <th>Size</th>
          <th>Created</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="img in images"
          :key="img.id"
        >
          <td>{{ img.tags.length > 0 ? img.tags.join(", ") : img.id.slice(7, 19) }}</td>
          <td>{{ formatSize(img.size) }}</td>
          <td>{{ formatCreated(img.created) }}</td>
          <td class="actions">
            <button
              :disabled="busy"
              @click="$emit('run', img.tags[0] ?? img.id)"
            >
              Run
            </button>
            <button
              :disabled="busy"
              @click="$emit('remove', img.tags[0] ?? img.id)"
            >
              Remove
            </button>
          </td>
        </tr>
        <tr v-if="images.length === 0">
          <td
            colspan="4"
            class="empty"
          >
            No images.
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.image-table table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}
th,
td {
  text-align: left;
  padding: 0.4rem 0.6rem;
  border-bottom: 1px solid var(--color-border);
}
.pull-form {
  display: flex;
  gap: 0.5rem;
  padding: 0.5rem 0.6rem;
}
.pull-form input {
  flex: 1;
}
.actions button {
  padding: 0.2rem 0.6rem;
  font-size: 0.8rem;
}
.empty {
  opacity: 0.6;
}
</style>
