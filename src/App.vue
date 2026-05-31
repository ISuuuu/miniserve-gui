<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Download, VideoPlay, VideoPause, Refresh, InfoFilled, Setting, Sunny, Moon } from "@element-plus/icons-vue";
import { getVersion } from "@tauri-apps/api/app";
import { useI18n } from "vue-i18n";
import ConfigPanel from "./components/ConfigPanel.vue";
import StatusCard from "./components/StatusCard.vue";
import LogPanel from "./components/LogPanel.vue";
import { useLogs } from "./composables/useLogs";
import { useEngine } from "./composables/useEngine";
import { useConfig } from "./composables/useConfig";
import { useServer } from "./composables/useServer";
import { useUpdater } from "./composables/useUpdater";

const { t } = useI18n();

// ============ Composables ============

const logsModule = useLogs();
const engineModule = useEngine();
const configModule = useConfig();
const serverModule = useServer(
  configModule.config,
  () => !!engineModule.engineStatus.value?.exists,
  logsModule,
);
const updaterModule = useUpdater(
  () => appVersion.value,
  logsModule,
);

// ============ Local State ============

const copySuccessIdx = ref<Set<number>>(new Set());
const copyTimers = new Map<number, ReturnType<typeof setTimeout>>();
const hoveredIdx = ref<number | null>(null);
const hoveredFeature = ref("");
const appVersion = ref("");
const aboutVisible = ref(false);
const settingsVisible = ref(false);
const isDark = ref(localStorage.getItem("theme") === "dark" || (!localStorage.getItem("theme") && window.matchMedia("(prefers-color-scheme: dark)").matches));

function applyTheme() {
  if (isDark.value) {
    document.documentElement.classList.add("dark");
  } else {
    document.documentElement.classList.remove("dark");
  }
}

function toggleTheme() {
  isDark.value = !isDark.value;
  localStorage.setItem("theme", isDark.value ? "dark" : "light");
  applyTheme();
}

// ============ Clipboard & URL ============

async function copyUrl(url?: string, idx?: number) {
  const urlToCopy = url || serverModule.serverStatus.value?.url || "";
  if (!urlToCopy) return;
  try {
    await navigator.clipboard.writeText(urlToCopy);
    if (idx !== undefined) {
      copySuccessIdx.value = new Set([...copySuccessIdx.value, idx]);
      if (copyTimers.has(idx)) clearTimeout(copyTimers.get(idx));
      copyTimers.set(idx, setTimeout(() => {
        const next = new Set(copySuccessIdx.value);
        next.delete(idx);
        copySuccessIdx.value = next;
        copyTimers.delete(idx);
      }, 2000));
    }
    ElMessage.success(t("messages.linkCopied"));
  } catch {
    ElMessage.error(t("messages.copyFailed"));
  }
}

async function openUrl(url: string) {
  try {
    const { openUrl: tauriOpenUrl } = await import("@tauri-apps/plugin-opener");
    await tauriOpenUrl(url);
  } catch (e) {
    console.error("Failed to open URL:", e);
    ElMessage.error(t("messages.openUrlFailed", { error: e }));
  }
}

async function selectPath() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ directory: true, multiple: false });
  if (selected) {
    configModule.config.path = selected as string;
  }
}

// ============ Event Listeners ============

const unlistenFns: (() => void)[] = [];

onMounted(async () => {
  applyTheme();
  try {
    await invoke("show_window_command");
  } catch (e) {
    console.error("Failed to show window:", e);
  }



  try {
    appVersion.value = await getVersion();
  } catch (e) {
    console.warn("无法获取 Tauri 版本", e);
  }

  await engineModule.checkEngine();
  await configModule.loadConfig();

  unlistenFns.push(
    await listen<number>("download-progress", (event) => {
      engineModule.progress.value = event.payload;
    }),
  );

  unlistenFns.push(
    await listen("server-started", (event) => {
      logsModule.addLog("Server event: " + JSON.stringify(event.payload));
    }),
  );

  unlistenFns.push(
    await listen<string>("server-log", (event) => {
      logsModule.addLog(event.payload);
    }),
  );
});

onUnmounted(() => {
  configModule.cleanup();
  copyTimers.forEach((timer) => clearTimeout(timer));
  copyTimers.clear();
  unlistenFns.forEach((fn) => fn());
  unlistenFns.length = 0;
});
</script>

<template>
  <div class="app-container">
    <!-- Header -->
    <header class="app-header">
      <div class="header-left">
        <div class="header-buttons">
          <el-button
            type="success"
            :icon="VideoPlay"
            @click="serverModule.startServer"
            :loading="serverModule.loading.value"
            class="btn-start"
          >
            {{ serverModule.serverStatus.value?.running ? t('header.restart') : t('header.start') }}
          </el-button>
          <el-button
            v-if="serverModule.serverStatus.value?.running"
            type="danger"
            :icon="VideoPause"
            @click="serverModule.stopServer"
            :loading="serverModule.loading.value"
            class="btn-stop"
          >
            {{ t('header.stop') }}
          </el-button>
        </div>
      </div>
      <div class="header-actions">
        <el-tag v-if="engineModule.engineStatus.value?.exists" type="success" size="small" class="status-tag">
          {{ t('header.engineReady') }} {{ engineModule.engineStatus.value.version ? `(${engineModule.engineStatus.value.version})` : "" }}
        </el-tag>
        <el-tag v-else type="warning" size="small" class="status-tag">{{ t('header.engineNotInstalled') }}</el-tag>
        <el-button
          v-if="!engineModule.engineStatus.value?.exists"
          type="primary"
          size="small"
          :icon="Download"
          @click="engineModule.downloadEngine"
          :loading="engineModule.downloading.value"
          :disabled="serverModule.serverStatus.value?.running"
        >
          {{ engineModule.downloading.value ? t('header.downloading', { progress: engineModule.progress.value.toFixed(0) }) : t('header.downloadEngine') }}
        </el-button>
        <el-button
          v-else
          type="info"
          size="small"
          :icon="Refresh"
          @click="engineModule.downloadEngine"
          :loading="engineModule.downloading.value"
          :disabled="serverModule.serverStatus.value?.running"
        >
          {{ t('header.updateEngine') }}
        </el-button>

        <el-button
          circle
          size="small"
          :icon="isDark ? Sunny : Moon"
          class="theme-toggle-btn"
          @click="toggleTheme"
        />

        <el-popover trigger="click" :width="140" popper-class="header-menu-popover">
          <template #reference>
            <el-button circle size="small" :icon="Setting" />
          </template>
          <div class="header-menu-item" @click="settingsVisible = true">
            <el-icon><Setting /></el-icon> {{ t('header.settings') }}
          </div>
          <div class="header-menu-item" @click="aboutVisible = true">
            <el-icon><InfoFilled /></el-icon> {{ t('header.about') }}
          </div>
        </el-popover>
      </div>
    </header>

    <!-- 设置 Dialog -->
    <el-dialog v-model="settingsVisible" :title="t('settings.title')" width="440px" align-center class="premium-dialog">
      <el-form label-width="125" size="default" style="padding: 10px 0;">
        <el-form-item :label="t('config.githubProxy')">
          <el-input
            v-model="configModule.config.github_proxy"
            :placeholder="t('config.githubProxyPlaceholder')"
          />
          <div class="form-item-hint">
            {{ t('config.githubProxyTooltip') }}
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="settingsVisible = false">{{ t('settings.close') }}</el-button>
      </template>
    </el-dialog>

    <!-- 关于软件 Dialog -->
    <el-dialog v-model="aboutVisible" :show-title="false" width="380px" align-center class="about-dialog premium-dialog">
      <div class="about-banner">
        <h3 class="about-name" @click="openUrl('https://github.com/ISuuuu/miniserve-gui')">miniserve-gui</h3>
        <el-tag type="info" size="small" effect="plain" round>{{ t('about.version', { version: appVersion || t('about.unknownVersion') }) }}</el-tag>
      </div>
      <div class="about-body">
        <p class="about-desc">
          {{ t('app.description') }}
        </p>
        <p class="about-based">
          <i18n-t keypath="app.basedOn" tag="span">
            <template #link>
              <a href="#" @click.prevent="openUrl('https://github.com/svenstaro/miniserve')" class="about-link">svenstaro/miniserve</a>
            </template>
          </i18n-t>
        </p>
      </div>
      <template #footer>
        <div class="about-footer">
          <el-button
            type="primary"
            @click="updaterModule.checkForUpdates"
            :loading="updaterModule.checkingUpdate.value"
          >{{ t('about.checkUpdate') }}</el-button>
          <el-button @click="aboutVisible = false">{{ t('about.close') }}</el-button>
        </div>
      </template>
    </el-dialog>

    <el-progress
      v-if="engineModule.downloading.value"
      :percentage="engineModule.progress.value"
      :format="(p: number) => `${p.toFixed(1)}%`"
      class="download-progress"
    />

    <el-progress
      v-if="updaterModule.updateDownloading.value"
      :percentage="updaterModule.updateProgress.value"
      :format="(p: number) => t('update.progressLabel', { percent: p.toFixed(1) })"
      class="download-progress"
    />

    <div class="main-layout">
      <!-- Config Panel -->
      <ConfigPanel
        :config="configModule.config"
        :hovered-feature="hoveredFeature"
        @select-path="selectPath"
        @update:hovered-feature="hoveredFeature = $event"
      />

      <!-- Right Panel: QR + Logs -->
      <main class="right-panel">
        <!-- Server Status Card -->
        <StatusCard
          :server-status="serverModule.serverStatus.value"
          :server-urls="serverModule.serverUrls.value"
          :qr-codes="serverModule.qrCodes.value"
          :copy-success-idx="copySuccessIdx"
          :hovered-idx="hoveredIdx"
          @copy-url="copyUrl"
          @open-url="openUrl"
          @hover-url="hoveredIdx = $event"
        />

        <!-- Log Panel -->
        <LogPanel
          :logs="logsModule.logs.value"
          @clear-logs="logsModule.clearLogs"
        />
      </main>
    </div>
  </div>
</template>

<style>
:root {
  --primary-color: #409eff;
  --primary-hover: #66b1ff;
  --primary-light: #ecf5ff;
  --bg-app: #f5f7fa;
  --bg-card: #ffffff;
  --bg-glass: rgba(255, 255, 255, 0.85);
  --border-color: #e4e7ed;
  --text-main: #303133;
  --text-muted: #909399;
  --text-hint: #c0c4cc;
  
  /* Pill Specific */
  --bg-pill: #f4f4f5;
  --bg-pill-hover: #e4e4e7;
  --text-pill: #3f3f46;
  --dot-pill: #d1d1d6;
  
  /* URL item Specific */
  --bg-url-item: #f5f7fa;
  --bg-url-item-hover: #ecf5ff;
  --bg-qr: #fafafa;
  
  --shadow-sm: 0 2px 12px 0 rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 16px 0 rgba(0, 0, 0, 0.08);
  --shadow-lg: 0 8px 24px 0 rgba(0, 0, 0, 0.12);
  --transition-speed: 0.2s;
}

:root.dark {
  --primary-color: #38bdf8;
  --primary-hover: #7dd3fc;
  --primary-light: rgba(56, 189, 248, 0.1);
  --bg-app: #090d16;
  --bg-card: #131924;
  --bg-glass: rgba(19, 25, 36, 0.85);
  --border-color: #222b3c;
  --text-main: #f8fafc;
  --text-muted: #94a3b8;
  --text-hint: #475569;
  
  /* Pill Specific */
  --bg-pill: #1e293b;
  --bg-pill-hover: #334155;
  --text-pill: #e2e8f0;
  --dot-pill: #475569;
  
  /* URL item Specific */
  --bg-url-item: #1e293b;
  --bg-url-item-hover: rgba(56, 189, 248, 0.15);
  --bg-qr: #1e293b;
  
  --shadow-sm: 0 2px 12px 0 rgba(0, 0, 0, 0.3);
  --shadow-md: 0 4px 16px 0 rgba(0, 0, 0, 0.4);
  --shadow-lg: 0 8px 24px 0 rgba(0, 0, 0, 0.5);

  /* Element Plus custom overrides to match our rich theme */
  --el-bg-color: #090d16 !important;
  --el-bg-color-overlay: #131924 !important;
  --el-border-color-light: #222b3c !important;
  --el-text-color-primary: #f8fafc !important;
  --el-text-color-regular: #94a3b8 !important;
  --el-color-primary: #38bdf8 !important;
  --el-color-primary-light-9: rgba(56, 189, 248, 0.1) !important;
  --el-color-success: #10b981 !important;
  --el-color-danger: #f43f5e !important;
}

html, body {
  margin: 0;
  padding: 0;
  overflow: hidden;
  height: 100%;
  background-color: var(--bg-app);
  color: var(--text-main);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  transition: background-color var(--transition-speed) ease, border-color var(--transition-speed) ease;
}

/* Webkit 自定义滚动条 - Mac 风格细长圆润 */
::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}

/* Dark mode overrides for config folder button and PID tag to prevent glare */
:root.dark .path-row .el-button {
  background: rgba(56, 189, 248, 0.15) !important;
  border-color: rgba(56, 189, 248, 0.3) !important;
  color: #38bdf8 !important;
}

:root.dark .path-row .el-button:hover {
  background: rgba(56, 189, 248, 0.25) !important;
  border-color: rgba(56, 189, 248, 0.5) !important;
}

:root.dark .pid-tag {
  background: rgba(16, 185, 129, 0.15) !important;
  border-color: rgba(16, 185, 129, 0.3) !important;
  color: #34d399 !important;
}

/* Dark mode overrides for QR code to reduce white glare */
:root.dark .qr-img {
  filter: brightness(0.85);
  opacity: 0.85;
  transition: all 0.2s ease;
}

:root.dark .qr-column:hover .qr-img {
  filter: brightness(1);
  opacity: 1;
}

/* Dark mode overrides for dialog buttons to make them softer */
:root.dark .premium-dialog .el-button--primary {
  background: rgba(56, 189, 248, 0.15) !important;
  border-color: rgba(56, 189, 248, 0.3) !important;
  color: #38bdf8 !important;
}

:root.dark .premium-dialog .el-button--primary:hover {
  background: rgba(56, 189, 248, 0.25) !important;
  border-color: rgba(56, 189, 248, 0.5) !important;
}

:root.dark .premium-dialog .el-button:not(.el-button--primary) {
  background: rgba(255, 255, 255, 0.05) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
  color: var(--text-main) !important;
}

:root.dark .premium-dialog .el-button:not(.el-button--primary):hover {
  background: rgba(255, 255, 255, 0.1) !important;
  border-color: rgba(255, 255, 255, 0.2) !important;
}

.form-item-hint {
  font-size: 11px;
  line-height: 1.4;
  color: var(--text-muted);
  margin-top: 6px;
}
</style>

<style scoped>
.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-app);
  overflow: hidden;
  transition: background-color var(--transition-speed) ease;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: var(--bg-card);
  border-bottom: 1px solid var(--border-color);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
  z-index: 10;
  transition: background-color var(--transition-speed) ease, border-color var(--transition-speed) ease;
}

.header-left {
  display: flex;
  align-items: center;
}

.header-buttons {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-start {
  background: #67c23a;
  border-color: #67c23a;
  color: #fff;
  transition: all 0.2s ease;
}

.btn-start:hover {
  background: #85ce61;
  border-color: #85ce61;
  transform: translateY(-0.5px);
}

.btn-stop {
  background: #f56c6c;
  border-color: #f56c6c;
  color: #fff;
  transition: all 0.2s ease;
}

.btn-stop:hover {
  background: #f78989;
  border-color: #f78989;
  transform: translateY(-0.5px);
}

/* Dark mode overrides for start/stop buttons to prevent excessive brightness */
:root.dark .btn-start {
  background: rgba(16, 185, 129, 0.15) !important;
  border-color: rgba(16, 185, 129, 0.3) !important;
  color: #34d399 !important;
}

:root.dark .btn-start:hover {
  background: rgba(16, 185, 129, 0.25) !important;
  border-color: rgba(16, 185, 129, 0.5) !important;
}

:root.dark .btn-stop {
  background: rgba(244, 63, 94, 0.15) !important;
  border-color: rgba(244, 63, 94, 0.3) !important;
  color: #fb7185 !important;
}

:root.dark .btn-stop:hover {
  background: rgba(244, 63, 94, 0.25) !important;
  border-color: rgba(244, 63, 94, 0.5) !important;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-tag {
  border-radius: 6px;
  font-weight: 500;
  padding: 4px 8px;
}

.theme-toggle-btn {
  background: transparent;
  border-color: var(--border-color);
  color: var(--text-main);
  transition: all 0.25s ease;
}

.theme-toggle-btn:hover {
  background: var(--border-color);
  color: var(--primary-color);
  transform: rotate(20deg);
}

.header-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-main);
  transition: all 0.2s ease;
}

.header-menu-item .el-icon {
  font-size: 15px;
  color: var(--text-muted);
  transition: color 0.2s ease;
}

.header-menu-item:hover {
  background: var(--primary-light);
  color: var(--primary-color);
}

.header-menu-item:hover .el-icon {
  color: var(--primary-color);
}

.download-progress {
  padding: 0 20px;
}

.main-layout {
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 0;
}

.right-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px 16px;
  overflow: hidden;
  min-height: 0;
}

.about-banner {
  text-align: center;
  padding: 32px 20px 24px;
  background: linear-gradient(135deg, var(--primary-color) 0%, #1d4ed8 100%);
  border-radius: 8px 8px 0 0;
  margin: -20px -20px 0;
  position: relative;
  overflow: hidden;
}

:root.dark .about-banner {
  background: linear-gradient(135deg, #090d16 0%, #131924 100%);
  border-bottom: 1px solid var(--border-color);
}

.about-banner::before {
  content: "";
  position: absolute;
  top: -50%;
  left: -50%;
  width: 200%;
  height: 200%;
  background: radial-gradient(circle, rgba(255, 255, 255, 0.15) 0%, transparent 60%);
  animation: banner-glow 6s infinite linear;
  pointer-events: none;
}

.about-icon,
.about-name,
.about-banner .el-tag {
  position: relative;
  z-index: 2;
}

@keyframes banner-glow {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.about-icon {
  font-size: 40px;
  margin-bottom: 12px;
  filter: drop-shadow(0 4px 6px rgba(0, 0, 0, 0.1));
}

.about-name {
  color: #fff;
  font-size: 20px;
  font-weight: 800;
  margin: 0 0 10px;
  cursor: pointer;
  transition: opacity 0.2s;
  letter-spacing: 0.5px;
}

.about-name:hover {
  opacity: 0.85;
}

.about-banner .el-tag {
  background: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.3);
  color: #fff;
  backdrop-filter: blur(4px);
}

.about-body {
  text-align: center;
  padding: 24px 8px 12px;
}

.about-desc {
  font-size: 13px;
  color: var(--text-muted);
  line-height: 1.6;
  margin: 0 0 10px;
}

.about-based {
  font-size: 12px;
  color: var(--text-muted);
  opacity: 0.8;
  margin: 0;
}

.about-link {
  color: var(--primary-color);
  text-decoration: none;
  font-weight: 600;
}

.about-link:hover {
  text-decoration: underline;
}

.about-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

/* Global modifications for premium dialog */
:deep(.premium-dialog) {
  border-radius: 8px !important;
  overflow: hidden;
  background: var(--bg-card) !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: var(--shadow-lg) !important;
}

:deep(.premium-dialog .el-dialog__header) {
  padding: 16px 20px 10px;
  margin-right: 0;
  border-bottom: 1px solid var(--border-color);
}

:deep(.premium-dialog .el-dialog__title) {
  font-size: 16px;
  font-weight: 700;
  color: var(--text-main);
}

:deep(.premium-dialog .el-dialog__body) {
  padding: 20px !important;
  background: var(--bg-card);
  color: var(--text-main);
}

:deep(.premium-dialog .el-dialog__footer) {
  padding: 10px 20px 16px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-card);
}
</style>

<style>
.header-menu-popover {
  padding: 6px !important;
  min-width: 140px !important;
  border-radius: 10px !important;
  background: var(--bg-card) !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: var(--shadow-md) !important;
}
</style>
