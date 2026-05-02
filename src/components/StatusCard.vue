<script setup lang="ts">
import { DocumentCopy, Cpu } from "@element-plus/icons-vue";
import { useI18n } from "vue-i18n";

interface ServerStatus {
  running: boolean;
  pid: number | null;
  url: string | null;
  urls: string[];
  port: number | null;
}

defineProps<{
  serverStatus: ServerStatus | null;
  serverUrls: string[];
  qrCodes: string[];
  copySuccessIdx: Set<number>;
  hoveredIdx: number | null;
}>();

const emit = defineEmits<{
  copyUrl: [url: string, idx: number];
  openUrl: [url: string];
  hoverUrl: [idx: number | null];
}>();

const { t } = useI18n();

function copyUrl(url: string, idx: number) {
  emit("copyUrl", url, idx);
}

function openUrl(url: string) {
  emit("openUrl", url);
}

function hoverUrl(idx: number | null) {
  emit("hoverUrl", idx);
}
</script>

<template>
  <el-card v-if="serverStatus?.running" class="status-card" shadow="hover">
    <template #header>
      <div class="card-header">
        <span><el-icon><Cpu /></el-icon> {{ t('status.running') }}</span>
        <el-tag type="success" size="small">{{ t('status.pid', { pid: serverStatus.pid }) }}</el-tag>
      </div>
    </template>
    <div class="url-layout">
      <div class="url-column">
        <div 
          v-for="(url, idx) in serverUrls" 
          :key="idx" 
          class="url-item"
          :class="{ active: hoveredIdx === idx }"
          @mouseenter="hoverUrl(idx)"
          @mouseleave="hoverUrl(null)"
        >
          <el-link type="primary" :href="url" :underline="false" @click.prevent="openUrl(url)">
            {{ url }}
          </el-link>
          <el-button type="primary" size="small" text @click="copyUrl(url, idx)">
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
  transition: all 0.3s ease;
}

.status-card:hover {
  box-shadow: 0 4px 20px rgba(64, 158, 255, 0.15);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
}

.url-layout {
  display: flex;
  gap: 12px;
  min-height: auto;
  align-items: flex-start;
  justify-content: space-between;
}

.url-column {
  flex: 0 0 70%;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.url-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: #f5f7fa;
  border: 1px solid transparent;
  border-radius: 6px;
  gap: 8px;
  font-size: 12px;
  transition: all 0.2s ease;
  width: 100%;
  cursor: pointer;
}

.url-item:hover {
  background: #ecf5ff;
}

.url-item .el-link {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.url-item .el-link:hover {
  text-decoration: none;
}

.url-item .el-button {
  transition: all 0.2s ease;
}

.url-item .el-button:hover {
  color: #409EFF;
}

.url-item.active {
  background: #ecf5ff;
}

.qr-column {
  width: 150px;
  height: 150px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #fafafa;
  border: 1px solid #eee;
  border-radius: 8px;
  align-self: center;
  position: sticky;
  top: 10px;
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
  color: #909399;
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
  opacity: 0.15;
  color: #303133;
}

.idle-state {
  text-align: center;
  padding: 16px;
  color: #909399;
}

.idle-state .hint {
  font-size: 13px;
  margin-top: 8px;
  color: #c0c4cc;
}
</style>
