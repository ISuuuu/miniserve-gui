<script setup lang="ts">
import { DocumentTextOutline } from "@vicons/ionicons5";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  logs: string[];
}>();

const emit = defineEmits<{
  clearLogs: [];
}>();

const { t } = useI18n();
const logBoxRef = ref<HTMLElement | null>(null);

watch(() => props.logs.length, () => {
  if (logBoxRef.value) {
    logBoxRef.value.scrollTop = logBoxRef.value.scrollHeight;
  }
}, { flush: 'post' });

function clearLogs() {
  emit("clearLogs");
}

function getLogClass(log: string) {
  if (log.includes("[ERROR]") || log.toLowerCase().includes("error") || log.toLowerCase().includes("failed")) return "log-error";
  if (log.includes("[WARN]") || log.toLowerCase().includes("warn")) return "log-warn";
  if (log.includes("[INFO]") || log.includes("Server event") || log.toLowerCase().includes("started")) return "log-info";
  if (log.includes("http://")) return "log-success";
  return "";
}
</script>

<template>
  <div class="log-card">
    <div class="card-header">
      <span><n-icon size="14" style="margin-right: 4px;"><DocumentTextOutline /></n-icon> {{ t('log.title') }}</span>
      <n-button text size="small" @click="clearLogs">{{ t('log.clear') }}</n-button>
    </div>
    <div ref="logBoxRef" class="log-box">
      <template v-if="logs.length === 0">
        <p class="log-empty-prompt"><span class="prompt-symbol">$</span> {{ t('log.empty') }}<span class="cursor-blink">▊</span></p>
      </template>
      <template v-else>
        <p v-for="(log, i) in logs" :key="i" class="log-line" :class="getLogClass(log)">{{ log }}</p>
      </template>
    </div>
  </div>
</template>

<style scoped>
.log-card {
  flex: 1 1 350px;
  display: flex;
  flex-direction: column;
  border-radius: 8px;
  overflow: hidden;
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  transition: all var(--transition-speed) ease;
  min-height: 0;
}

.log-card:hover {
  box-shadow: var(--shadow-md);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
  font-size: 14px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-color);
}

.log-box {
  background: #0f172a;
  color: #e2e8f0;
  padding: 16px 20px;
  font-family: "Fira Code", "Consolas", "Monaco", monospace;
  font-size: 12px;
  line-height: 1.8;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  border-bottom-left-radius: 8px;
  border-bottom-right-radius: 8px;
}

.log-line {
  margin: 4px 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.log-error {
  color: #f87171;
}

.log-warn {
  color: #fbbf24;
}

.log-info {
  color: #38bdf8;
}

.log-success {
  color: #34d399;
}

.log-empty-prompt {
  color: #475569;
}

.prompt-symbol {
  color: #6366f1;
  margin-right: 8px;
  font-weight: 600;
}

.cursor-blink {
  color: #34d399;
  text-shadow: 0 0 6px rgba(52, 211, 153, 0.6);
  animation: blink 1s step-end infinite;
  margin-left: 2px;
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}
</style>
