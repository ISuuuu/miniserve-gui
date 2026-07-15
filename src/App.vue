<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { darkTheme } from "naive-ui";
import { isDarkTheme, message } from "./utils/discrete";
import {
  DownloadOutline,
  PlayOutline,
  PauseOutline,
  RefreshOutline,
  SettingsOutline,
  SunnyOutline,
  MoonOutline,
} from "@vicons/ionicons5";
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

const theme = computed(() => (isDarkTheme.value ? darkTheme : null));

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

const menuOptions = computed(() => [
  { label: t('header.settings'), key: 'settings' },
  { label: t('header.about'), key: 'about' },
]);

function onMenuSelect(key: string) {
  if (key === 'settings') settingsVisible.value = true;
  else if (key === 'about') aboutVisible.value = true;
}

function applyTheme() {
  if (isDarkTheme.value) {
    document.documentElement.classList.add("dark");
  } else {
    document.documentElement.classList.remove("dark");
  }
}

function toggleTheme() {
  isDarkTheme.value = !isDarkTheme.value;
  localStorage.setItem("theme", isDarkTheme.value ? "dark" : "light");
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
    message.success(t("messages.linkCopied"));
  } catch {
    message.error(t("messages.copyFailed"));
  }
}

async function openUrl(url: string) {
  try {
    const { openUrl: tauriOpenUrl } = await import("@tauri-apps/plugin-opener");
    await tauriOpenUrl(url);
  } catch (e) {
    console.error("Failed to open URL:", e);
    message.error(t("messages.openUrlFailed", { error: e }));
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
  <n-config-provider :theme="theme">
    <n-dialog-provider>
      <n-message-provider>
        <div class="app-container">
          <!-- Header -->
          <header class="app-header">
            <div class="header-left">
              <div class="header-buttons">
                <n-button
                  type="success"
                  :loading="serverModule.loading.value"
                  class="btn-start"
                  @click="serverModule.startServer"
                >
                  <template #icon>
                    <n-icon><PlayOutline /></n-icon>
                  </template>
                  {{ serverModule.serverStatus.value?.running ? t('header.restart') : t('header.start') }}
                </n-button>
                <n-button
                  v-if="serverModule.serverStatus.value?.running"
                  type="error"
                  :loading="serverModule.loading.value"
                  class="btn-stop"
                  @click="serverModule.stopServer"
                >
                  <template #icon>
                    <n-icon><PauseOutline /></n-icon>
                  </template>
                  {{ t('header.stop') }}
                </n-button>
              </div>
            </div>
            <div class="header-actions">
              <n-tag v-if="engineModule.engineStatus.value?.exists" :bordered="false" type="success" size="small" class="status-tag">
                {{ t('header.engineReady') }} {{ engineModule.engineStatus.value.version ? `(${engineModule.engineStatus.value.version})` : "" }}
              </n-tag>
              <n-tag v-else :bordered="false" type="warning" size="small" class="status-tag">{{ t('header.engineNotInstalled') }}</n-tag>
              <n-button
                v-if="!engineModule.engineStatus.value?.exists"
                type="primary"
                size="small"
                :loading="engineModule.downloading.value"
                :disabled="serverModule.serverStatus.value?.running"
                @click="engineModule.downloadEngine"
              >
                <template #icon>
                  <n-icon><DownloadOutline /></n-icon>
                </template>
                {{ engineModule.downloading.value ? t('header.downloading', { progress: engineModule.progress.value.toFixed(0) }) : t('header.downloadEngine') }}
              </n-button>
              <n-button
                v-else
                type="default"
                size="small"
                :loading="engineModule.downloading.value"
                :disabled="serverModule.serverStatus.value?.running"
                @click="engineModule.downloadEngine"
              >
                <template #icon>
                  <n-icon><RefreshOutline /></n-icon>
                </template>
                {{ t('header.updateEngine') }}
              </n-button>

              <n-button
                circle
                secondary
                size="small"
                class="theme-toggle-btn"
                @click="toggleTheme"
              >
                <template #icon>
                  <n-icon><SunnyOutline v-if="isDarkTheme" /><MoonOutline v-else /></n-icon>
                </template>
              </n-button>

              <n-dropdown :options="menuOptions" @select="onMenuSelect" trigger="click" placement="bottom-end">
                <n-button circle secondary size="small">
                  <template #icon>
                    <n-icon><SettingsOutline /></n-icon>
                  </template>
                </n-button>
              </n-dropdown>
            </div>
          </header>

          <!-- 设置 Modal -->
          <n-modal v-model:show="settingsVisible" preset="card" :title="t('settings.title')" style="width: 440px" class="premium-modal">
            <div style="padding: 10px 0;">
              <div class="form-row">
                <label class="form-label" style="width: 125px;">{{ t('config.githubProxy') }}</label>
                <div class="form-control">
                  <n-input
                    :value="configModule.config.github_proxy"
                    :placeholder="t('config.githubProxyPlaceholder')"
                    @update:value="configModule.config.github_proxy = $event"
                  />
                  <div class="form-item-hint">
                    {{ t('config.githubProxyTooltip') }}
                  </div>
                </div>
              </div>
            </div>
            <template #footer>
              <n-button @click="settingsVisible = false">{{ t('settings.close') }}</n-button>
            </template>
          </n-modal>

          <!-- 关于软件 Modal -->
          <n-modal v-model:show="aboutVisible" style="width: 380px" class="about-modal">
            <div class="about-content">
              <div class="about-banner">
                <h3 class="about-name" @click="openUrl('https://github.com/ISuuuu/miniserve-gui')">miniserve-gui</h3>
                <n-tag :bordered="false" type="info" size="small" round>{{ t('about.version', { version: appVersion || t('about.unknownVersion') }) }}</n-tag>
              </div>
              <div class="about-body">
                <p class="about-desc">
                  {{ t('app.description') }}
                </p>
                <p class="about-based">
                  {{ t('app.basedOnPrefix') }}
                  <a href="#" @click.prevent="openUrl('https://github.com/svenstaro/miniserve')" class="about-link">svenstaro/miniserve</a>
                </p>
              </div>
              <div class="about-footer">
                <n-button
                  type="primary"
                  :loading="updaterModule.checkingUpdate.value"
                  @click="updaterModule.checkForUpdates"
                >{{ t('about.checkUpdate') }}</n-button>
                <n-button @click="aboutVisible = false">{{ t('about.close') }}</n-button>
              </div>
            </div>
          </n-modal>

          <n-progress
            v-if="engineModule.downloading.value"
            type="line"
            :percentage="engineModule.progress.value"
            :show-percentage="false"
            :processing="engineModule.downloading.value"
            class="download-progress"
          >
            <template #default>
              <span style="font-size: 12px; color: var(--text-muted);">{{ `${engineModule.progress.value.toFixed(1)}%` }}</span>
            </template>
          </n-progress>

          <n-progress
            v-if="updaterModule.updateDownloading.value"
            type="line"
            :percentage="updaterModule.updateProgress.value"
            :show-percentage="false"
            :processing="updaterModule.updateDownloading.value"
            class="download-progress"
          >
            <template #default>
              <span style="font-size: 12px; color: var(--text-muted);">{{ t('update.progressLabel', { percent: updaterModule.updateProgress.value.toFixed(1) }) }}</span>
            </template>
          </n-progress>

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
      </n-message-provider>
    </n-dialog-provider>
  </n-config-provider>
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
:root.dark .path-row .n-button {
  --n-color: rgba(56, 189, 248, 0.15) !important;
  --n-border: rgba(56, 189, 248, 0.3) !important;
  --n-text-color: #38bdf8 !important;
}

:root.dark .pid-tag {
  --n-color: rgba(16, 185, 129, 0.15) !important;
  --n-border: rgba(16, 185, 129, 0.3) !important;
  --n-text-color: #34d399 !important;
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

.form-item-hint {
  font-size: 11px;
  line-height: 1.4;
  color: var(--text-muted);
  margin-top: 6px;
}

.form-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.form-label {
  flex-shrink: 0;
  width: 90px;
  font-size: 13px;
  color: var(--text-muted);
  font-weight: 500;
  text-align: right;
  padding-top: 6px;
}

.form-control {
  flex: 1;
  min-width: 0;
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
  --n-color: #67c23a !important;
  --n-color-hover: #85ce61 !important;
  --n-border: #67c23a !important;
  --n-border-hover: #85ce61 !important;
  --n-text-color: #fff !important;
  --n-text-color-hover: #fff !important;
}

.btn-stop {
  --n-color: #f56c6c !important;
  --n-color-hover: #f78989 !important;
  --n-border: #f56c6c !important;
  --n-border-hover: #f78989 !important;
  --n-text-color: #fff !important;
  --n-text-color-hover: #fff !important;
}

/* Dark mode overrides for start/stop buttons to prevent excessive brightness */
:root.dark .btn-start {
  --n-color: rgba(16, 185, 129, 0.15) !important;
  --n-color-hover: rgba(16, 185, 129, 0.25) !important;
  --n-border: rgba(16, 185, 129, 0.3) !important;
  --n-border-hover: rgba(16, 185, 129, 0.5) !important;
  --n-text-color: #34d399 !important;
  --n-text-color-hover: #34d399 !important;
}

:root.dark .btn-stop {
  --n-color: rgba(244, 63, 94, 0.15) !important;
  --n-color-hover: rgba(244, 63, 94, 0.25) !important;
  --n-border: rgba(244, 63, 94, 0.3) !important;
  --n-border-hover: rgba(244, 63, 94, 0.5) !important;
  --n-text-color: #fb7185 !important;
  --n-text-color-hover: #fb7185 !important;
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
  transition: all 0.25s ease;
}

.theme-toggle-btn:hover {
  transform: rotate(20deg);
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

.about-modal :deep(.n-card__content) {
  padding: 0 !important;
}

.about-modal :deep(.n-card__footer) {
  padding: 12px 20px !important;
  border-top: 1px solid var(--border-color);
  display: block !important;
  visibility: visible !important;
}

.about-content {
  background: var(--bg-card);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--shadow-lg);
  box-sizing: border-box;
}

.about-banner {
  text-align: center;
  padding: 40px 24px 32px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  position: relative;
  overflow: hidden;
}

:root.dark .about-banner {
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
  border-bottom: 1px solid var(--border-color);
}

.about-banner::before {
  content: "";
  position: absolute;
  top: -50%;
  left: -50%;
  width: 200%;
  height: 200%;
  background: radial-gradient(circle, rgba(255, 255, 255, 0.1) 0%, transparent 50%);
  animation: banner-glow 8s infinite linear;
  pointer-events: none;
}

.about-banner::after {
  content: "";
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 60px;
  background: linear-gradient(to top, rgba(0,0,0,0.1), transparent);
  pointer-events: none;
}

.about-name,
.about-banner .n-tag {
  position: relative;
  z-index: 2;
}

@keyframes banner-glow {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.about-name {
  color: #fff;
  font-size: 24px;
  font-weight: 800;
  margin: 0 0 12px;
  cursor: pointer;
  transition: all 0.2s;
  letter-spacing: 1px;
  text-shadow: 0 2px 4px rgba(0,0,0,0.2);
}

.about-name:hover {
  opacity: 0.9;
  transform: scale(1.02);
}

.about-banner .n-tag {
  background: rgba(255, 255, 255, 0.2);
  color: #fff;
  backdrop-filter: blur(8px);
  border: 1px solid rgba(255,255,255,0.3);
  font-weight: 500;
}

.about-body {
  text-align: center;
  padding: 28px 24px 20px;
}

.about-desc {
  font-size: 14px;
  color: var(--text-main);
  line-height: 1.7;
  margin: 0 0 16px;
  font-weight: 400;
}

.about-based {
  font-size: 13px;
  color: var(--text-muted);
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
  padding: 16px 24px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-app);
  box-sizing: border-box;
}
</style>
