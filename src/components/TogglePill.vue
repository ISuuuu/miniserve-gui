<script setup lang="ts">
defineProps<{
  modelValue: boolean;
  label: string;
  featureKey?: string;
}>();

defineEmits<{
  "update:modelValue": [value: boolean];
  hover: [key: string];
}>();
</script>

<template>
  <button
    type="button"
    class="toggle-pill"
    :class="{ active: modelValue }"
    @click="$emit('update:modelValue', !modelValue)"
    @mouseenter="featureKey && $emit('hover', featureKey)"
    @mouseleave="featureKey && $emit('hover', '')"
  >
    <span class="toggle-dot" />{{ label }}
  </button>
</template>

<style scoped>
.toggle-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  border-radius: 6px;
  border: none;
  background: var(--bg-pill);
  color: var(--text-pill);
  font-size: 11px;
  cursor: pointer;
  transition: background-color 0.2s ease, transform 0.1s ease;
  user-select: none;
  white-space: nowrap;
  outline: none;
}

.toggle-pill:hover {
  background: var(--bg-pill-hover);
}

.toggle-pill:active {
  transform: scale(0.97);
}

.toggle-pill.active {
  background: var(--bg-pill);
}

.toggle-pill.active:hover {
  background: var(--bg-pill-hover);
}

.toggle-pill.active .toggle-dot {
  background: var(--primary-color);
}

.toggle-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--dot-pill);
  transition: background-color 0.2s ease;
  flex-shrink: 0;
}
</style>
