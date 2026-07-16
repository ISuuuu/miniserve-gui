<script setup lang="ts">
import { CopyOutline, HardwareChipOutline } from "@vicons/ionicons5";
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
  <n-card v-if="serverStatus?.running" class="status-card" size="small">
    <template #header>
      <div class="card-header">
        <span class="header-title"><n-icon size="14" style="margin-right: 4px;"><HardwareChipOutline /></n-icon> {{ t('status.running') }}</span>
        <n-tag :bordered="false" type="success" size="small" class="pid-tag">{{ t('status.pid', { pid: serverStatus.pid }) }}</n-tag>
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
          <a :href="url" class="url-link" @click.prevent="$emit('openUrl', url)">
            {{ url }}
          </a>
          <n-button type="primary" size="tiny" text @click="$emit('copyUrl', url, idx)" class="copy-btn">
            <template #icon>
              <n-icon size="14"><CopyOutline /></n-icon>
            </template>
            {{ copySuccessIdx.has(idx) ? t('status.copied') : t('status.copy') }}
          </n-button>
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
  </n-card>

  <n-card v-else class="status-card" size="small">
    <div class="idle-state">
      <p><n-icon size="16" style="margin-right: 4px;"><HardwareChipOutline /></n-icon> {{ t('status.notRunning') }}</p>
      <p class="hint">{{ t('status.hint') }}</p>
    </div>
  </n-card>
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
  max-height: 50%;
  overflow-y: auto;
}

.status-card:hover {
  box-shadow: var(--shadow-md);
  border-color: rgba(79, 70, 229, 0.15);
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
  justify-content: center;
}

.url-column {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  align-items: center;
}

.url-item {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 6px 16px;
  background: var(--bg-url-item);
  border: 1px solid transparent;
  border-radius: 8px;
  gap: 8px;
  font-size: 12px;
  transition: all 0.2s ease;
  width: 100%;
  max-width: 400px;
  cursor: pointer;
  box-sizing: border-box;
}

.url-item:hover, .url-item.active {
  background: var(--bg-url-item-hover);
  border-color: rgba(79, 70, 229, 0.1);
}

.url-link {
  min-width: 0;
  overflow: hidden;
  color: var(--primary-color);
  text-decoration: none;
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  text-overflow: ellipsis;
  display: block;
}

.url-link:hover {
  text-decoration: underline;
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
