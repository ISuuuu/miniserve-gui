<script setup lang="ts">
import { Files } from "@element-plus/icons-vue";
import { ref, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  logs: string[];
}>();

const emit = defineEmits<{
  clearLogs: [];
}>();

const { t } = useI18n();
const logBoxRef = ref<HTMLElement | null>(null);

watch(() => props.logs.length, async () => {
  await nextTick();
  if (logBoxRef.value) {
    const lastLog = logBoxRef.value.querySelector('.log-line:last-child');
    if (lastLog) {
      lastLog.scrollIntoView({ behavior: 'smooth' });
    }
  }
});

function clearLogs() {
  emit("clearLogs");
}
</script>

<template>
  <el-card class="log-card" shadow="hover">
    <template #header>
      <div class="card-header">
        <span><el-icon><Files /></el-icon> {{ t('log.title') }}</span>
        <el-button text size="small" @click="clearLogs">{{ t('log.clear') }}</el-button>
      </div>
    </template>
    <div ref="logBoxRef" class="log-box">
      <p v-for="(log, i) in logs" :key="i" class="log-line">{{ log }}</p>
      <p v-if="logs.length === 0" class="log-empty">{{ t('log.empty') }}</p>
    </div>
  </el-card>
</template>

<style scoped>
.log-card {
  flex: 1 1 350px;
  display: flex;
  flex-direction: column;
  border-radius: 12px;
  overflow: hidden;
}

.log-card :deep(.el-card__body) {
  padding: 0;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
}

.log-box {
  background: #1e1e1e;
  color: #4ADE80;
  padding: 16px;
  border-radius: 0 0 12px 12px;
  font-family: "Consolas", "Monaco", monospace;
  font-size: 12px;
  line-height: 1.8;
  overflow-y: auto;
  flex: 1;
}

.log-line {
  margin: 2px 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.log-empty {
  color: #666;
  text-align: center;
  padding: 20px;
}
</style>
