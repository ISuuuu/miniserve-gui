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
  gap: 5px;
  padding: 5px 10px;
  border-radius: 6px;
  border: none;
  background: #F4F4F5;
  color: #3F3F46;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  white-space: nowrap;
  outline: none;
}

.toggle-pill:hover {
  background: #E4E4E7;
}

.toggle-pill.active {
  background: #F4F4F5;
  color: #3F3F46;
}

.toggle-pill.active:hover {
  background: #E4E4E7;
}

.toggle-pill.active .toggle-dot {
  background: #409EFF;
}

.toggle-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #A1A1AA;
  transition: all 0.2s ease;
  flex-shrink: 0;
}
</style>
