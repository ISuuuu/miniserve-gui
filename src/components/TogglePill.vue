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
  gap: 7px;
  padding: 5px 12px;
  border-radius: 10px;
  border: 1.5px solid transparent;
  background: var(--bg-pill);
  color: var(--text-pill);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.22s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  user-select: none;
  white-space: nowrap;
  outline: none;
  line-height: 1.4;
}

.toggle-pill:hover {
  background: var(--bg-pill-hover);
  transform: translateY(-1px);
  box-shadow: var(--shadow-sm);
}

.toggle-pill:active {
  transform: scale(0.96);
}

.toggle-pill.active {
  background: var(--pill-active-bg);
  border-color: var(--pill-active-border);
  color: var(--pill-active-text);
}

.toggle-pill.active:hover {
  background: var(--pill-active-bg-hover);
}

.toggle-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--dot-pill);
  transition: all 0.22s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  flex-shrink: 0;
}

.toggle-pill.active .toggle-dot {
  background: var(--primary-color);
  box-shadow: 0 0 0 3px var(--pill-active-dot-ring);
}
</style>
