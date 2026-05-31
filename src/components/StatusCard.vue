<script setup lang="ts">
import { DocumentCopy, Cpu } from "@element-plus/icons-vue";
import { useI18n } from "vue-i18n";
import type { ServerStatus } from "../types";

defineProps<{
  serverStatus: ServerStatus | null;
  serverUrls: string[];
  qrCodes: string[];
  copySuccessIdx: Set<number>;
  hoveredIdx: number | null;
}>();

defineEmits<{
  copyUrl: [url: string, idx: number];
  openUrl: [url: string];
  hoverUrl: [idx: number | null];
}>();

const { t } = useI18n();
</script>

<template>
  <el-card v-if="serverStatus?.running" class="status-card" shadow="hover">
    <template #header>
      <div class="card-header">
        <span class="header-title"><el-icon><Cpu /></el-icon> {{ t('status.running') }}</span>
        <el-tag type="success" size="small" effect="dark" class="pid-tag">{{ t('status.pid', { pid: serverStatus.pid }) }}</el-tag>
      </div>
    </template>
    <div class="url-layout">
      <div class="url-column">
        <div
          v-for="(url, idx) in serverUrls"
          :key="idx"
          class="url-item"
          :class="{ active: hoveredIdx === idx }"
          @mouseenter="$emit('hoverUrl', idx)"
          @mouseleave="$emit('hoverUrl', null)"
        >
          <el-link type="primary" :href="url" :underline="false" @click.prevent="$emit('openUrl', url)" class="url-link">
            {{ url }}
          </el-link>
          <el-button type="primary" size="small" text @click="$emit('copyUrl', url, idx)" class="copy-btn">
            <el-icon><DocumentCopy /></el-icon>
            {{ copySuccessIdx.has(idx) ? t('status.copied') : t('status.copy') }}
          </el-button>
        </div>
      </div>
      <div class="qr-column">
        <div v-if="hoveredIdx !== null && qrCodes[hoveredIdx]" class="qr-display">
          <img :src="qrCodes[hoveredIdx]" alt="QR" class="qr-img" />
        </div>
        <div v-else class="qr-placeholder">
          <svg class="qr-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <rect x="3" y="3" width="7" height="7" rx="1" />
            <rect x="14" y="3" width="7" height="7" rx="1" />
            <rect x="3" y="14" width="7" height="7" rx="1" />
            <rect x="14" y="14" width="3" height="3" />
            <rect x="18" y="14" width="3" height="3" />
            <rect x="14" y="18" width="3" height="3" />
            <rect x="18" y="18" width="3" height="3" />
          </svg>
          <span>{{ t('status.qrHint') }}</span>
        </div>
      </div>
    </div>
  </el-card>

  <el-card v-else class="status-card" shadow="hover">
    <div class="idle-state">
      <p><el-icon><Cpu /></el-icon> {{ t('status.notRunning') }}</p>
      <p class="hint">{{ t('status.hint') }}</p>
    </div>
  </el-card>
</template>

<style scoped>
.status-card {
  flex-shrink: 0;
  flex: 0 1 auto;
  border-radius: 8px;
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  transition: all var(--transition-speed) ease;
}

.status-card:hover {
  box-shadow: var(--shadow-md);
  border-color: rgba(64, 158, 255, 0.2);
}

:deep(.el-card__header) {
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-color);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-main);
  font-weight: 600;
  font-size: 14px;
}

.pid-tag {
  border-radius: 4px;
}

.url-layout {
  display: flex;
  gap: 20px;
  min-height: auto;
  align-items: center;
  justify-content: space-between;
}

.url-column {
  flex: 1;
  max-width: 480px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.url-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  background: var(--bg-url-item);
  border: 1px solid transparent;
  border-radius: 6px;
  gap: 8px;
  font-size: 12px;
  transition: background-color 0.2s ease;
  width: 100%;
  cursor: pointer;
}

.url-item:hover, .url-item.active {
  background: var(--bg-url-item-hover);
}

.url-link {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

:deep(.url-link .el-link__inner) {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.url-link:hover {
  color: var(--primary-hover);
}

.copy-btn {
  transition: all 0.2s ease;
  border-radius: 4px;
}

.qr-column {
  width: 150px;
  height: 150px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-qr);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  position: relative;
  overflow: hidden;
  align-self: center;
}

.qr-display {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

.qr-display .qr-img {
  width: 130px;
  height: 130px;
  border-radius: 8px;
  display: block;
}

.qr-placeholder {
  color: var(--text-muted);
  font-size: 11px;
  text-align: center;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px;
}

.qr-icon {
  width: 48px;
  height: 48px;
  opacity: 0.4;
  color: var(--text-muted);
}

.idle-state {
  text-align: center;
  padding: 16px;
  color: var(--text-muted);
}

.idle-state p {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 700;
  color: var(--text-main);
}

.idle-state .hint {
  font-size: 13px;
  margin-top: 8px;
  color: var(--text-hint);
  font-weight: normal;
}
</style>
