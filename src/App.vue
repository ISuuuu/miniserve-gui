<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { darkTheme } from "naive-ui";
import type { GlobalThemeOverrides } from "naive-ui";
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

const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const isDark = isDarkTheme.value;
  const primaryColor = isDark ? "#3b82f6" : "#2563eb";
  const primaryColorHover = isDark ? "#60a5fa" : "#3b82f6";
  const primaryColorPressed = isDark ? "#2563eb" : "#1d4ed8";
  const primaryColorSuppl = isDark ? "rgba(59, 130, 246, 0.15)" : "rgba(37, 99, 235, 0.08)";

  const successColor = "#10b981";
  const successColorHover = "#34d399";
  const successColorPressed = "#059669";
  const successColorSuppl = isDark ? "rgba(16, 185, 129, 0.15)" : "rgba(16, 185, 129, 0.08)";

  const errorColor = "#f43f5e";
  const errorColorHover = "#fda4af";
  const errorColorPressed = "#e11d48";
  const errorColorSuppl = isDark ? "rgba(244, 63, 94, 0.15)" : "rgba(244, 63, 94, 0.08)";

  return {
    common: {
      primaryColor,
      primaryColorHover,
      primaryColorPressed,
      primaryColorSuppl,
      successColor,
      successColorHover,
      successColorPressed,
      successColorSuppl,
      errorColor,
      errorColorHover,
      errorColorPressed,
      errorColorSuppl,
      borderRadius: "10px",
    },
    Card: {
      borderRadius: "12px",
    },
    Button: {
      borderRadiusMedium: "8px",
      borderRadiusSmall: "6px",
      borderRadiusTiny: "4px",
    },
    Input: {
      borderRadius: "8px",
    },
    Select: {
      peers: {
        InternalSelection: {
          borderRadius: "8px",
        },
      },
    },
  };
});

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

  // 1. 优先安全注册事件监听器，单项失败不阻断后续生命周期
  const safeListen = async <T>(event: string, handler: (e: { payload: T }) => void) => {
    try {
      const unlisten = await listen<T>(event, handler);
      unlistenFns.push(unlisten);
    } catch (e) {
      console.error(`Failed to register listener for ${event}:`, e);
    }
  };

  await safeListen<number>("download-progress", (event) => {
    engineModule.progress.value = event.payload;
  });

  await safeListen("server-started", (event) => {
    logsModule.addLog("Server event: " + JSON.stringify(event.payload));
  });

  await safeListen<string>("server-log", (event) => {
    logsModule.addLog(event.payload);
  });

  // 2. 并行执行相互独立的初始化请求
  await Promise.allSettled([
    (async () => {
      try {
        appVersion.value = await getVersion();
      } catch (e) {
        console.warn("无法获取 Tauri 版本", e);
      }
    })(),
    engineModule.checkEngine(),
    configModule.loadConfig(),
  ]);
});

onUnmounted(() => {
  configModule.cleanup();
  logsModule.cleanup();
  copyTimers.forEach((timer) => clearTimeout(timer));
  copyTimers.clear();
  unlistenFns.forEach((fn) => fn());
  unlistenFns.length = 0;
});
</script>

<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-dialog-provider>
      <n-message-provider>
        <div class="app-container">
          <!-- Header -->
          <header class="app-header">
            <div class="header-left">
              <div class="header-buttons">
                <n-button
                  type="success"
                  ghost
                  size="small"
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
                  ghost
                  size="small"
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
            <div class="settings-form">
              <label class="settings-label">{{ t('config.githubProxy') }}</label>
              <n-input
                :value="configModule.config.github_proxy"
                :placeholder="t('config.githubProxyPlaceholder')"
                @update:value="configModule.config.github_proxy = $event"
              />
              <div class="settings-hint">
                {{ t('config.githubProxyTooltip') }}
              </div>
            </div>
            <template #footer>
              <n-button @click="settingsVisible = false">{{ t('settings.close') }}</n-button>
            </template>
          </n-modal>

          <!-- 关于软件 Modal -->
          <n-modal v-model:show="aboutVisible" style="width: 380px" class="about-modal">
            <div class="about-content-modern">
              <div class="about-header-modern">
                <div class="about-icon-box">
                  <svg class="about-app-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="2" y="2" width="20" height="20" rx="4" />
                    <path d="M9 17V7l7 5z" />
                  </svg>
                </div>
                <h3 class="about-name-modern" @click="openUrl('https://github.com/ISuuuu/miniserve-gui')">miniserve-gui</h3>
                <n-tag :bordered="false" type="primary" size="small" round>v{{ appVersion || t('about.unknownVersion') }}</n-tag>
              </div>
              <div class="about-body-modern">
                <p class="about-desc">
                  {{ t('app.description') }}
                </p>
                <p class="about-based">
                  {{ t('app.basedOnPrefix') }}
                  <a href="#" @click.prevent="openUrl('https://github.com/svenstaro/miniserve')" class="about-link">svenstaro/miniserve</a>
                </p>
              </div>
              <div class="about-footer-modern">
                <n-button
                  type="primary"
                  ghost
                  size="small"
                  :loading="updaterModule.checkingUpdate.value"
                  @click="updaterModule.checkForUpdates"
                >{{ t('about.checkUpdate') }}</n-button>
                <n-button size="small" @click="aboutVisible = false">{{ t('about.close') }}</n-button>
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
  --primary-color: #2563eb;
  --primary-hover: #3b82f6;
  --primary-light: rgba(37, 99, 235, 0.05);
  --bg-app: #fafafa;
  --bg-card: #ffffff;
  --bg-glass: rgba(255, 255, 255, 0.8);
  --border-color: #e2e8f0;
  --text-main: #0f172a;
  --text-muted: #64748b;
  --text-hint: #94a3b8;

  /* Pill Specific */
  --bg-pill: #f1f5f9;
  --bg-pill-hover: #e2e8f0;
  --text-pill: #334155;
  --dot-pill: #94a3b8;
  --pill-active-bg: rgba(37, 99, 235, 0.06);
  --pill-active-bg-hover: rgba(37, 99, 235, 0.1);
  --pill-active-border: rgba(37, 99, 235, 0.25);
  --pill-active-text: #2563eb;
  --pill-active-dot-ring: rgba(37, 99, 235, 0.15);

  /* Section title */
  --bg-section-title: rgba(37, 99, 235, 0.06);

  /* URL item Specific */
  --bg-url-item: #f8fafc;
  --bg-url-item-hover: rgba(37, 99, 235, 0.05);
  --bg-qr: #fafafa;

  --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.05), 0 2px 4px -1px rgba(0, 0, 0, 0.03);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.05), 0 4px 6px -2px rgba(0, 0, 0, 0.03);
  --transition-speed: 0.2s;
}

:root.dark {
  --primary-color: #3b82f6;
  --primary-hover: #60a5fa;
  --primary-light: rgba(59, 130, 246, 0.1);
  --bg-app: #080c14;
  --bg-card: #0f172a;
  --bg-glass: rgba(15, 23, 42, 0.8);
  --border-color: #1e293b;
  --text-main: #f8fafc;
  --text-muted: #94a3b8;
  --text-hint: #475569;

  /* Pill Specific */
  --bg-pill: #1e293b;
  --bg-pill-hover: #334155;
  --text-pill: #cbd5e1;
  --dot-pill: #475569;
  --pill-active-bg: rgba(59, 130, 246, 0.1);
  --pill-active-bg-hover: rgba(59, 130, 246, 0.15);
  --pill-active-border: rgba(59, 130, 246, 0.3);
  --pill-active-text: #60a5fa;
  --pill-active-dot-ring: rgba(59, 130, 246, 0.15);

  /* Section title */
  --bg-section-title: rgba(59, 130, 246, 0.08);

  /* URL item Specific */
  --bg-url-item: #1e293b;
  --bg-url-item-hover: rgba(59, 130, 246, 0.12);
  --bg-qr: #1e293b;

  --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.2);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.3), 0 2px 4px -1px rgba(0, 0, 0, 0.2);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.4), 0 4px 6px -2px rgba(0, 0, 0, 0.3);
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

.settings-form {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 8px 8px;
  align-items: center;
  padding: 10px 0;
}

.settings-label {
  font-size: 13px;
  color: var(--text-muted);
  font-weight: 500;
  text-align: right;
  white-space: nowrap;
}

.settings-hint {
  grid-column: 1 / -1;
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 2px;
  line-height: 1.5;
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

.about-content-modern {
  background: var(--bg-card);
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-lg);
  box-sizing: border-box;
}

.about-header-modern {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px 24px 20px;
  background: var(--bg-card);
  border-bottom: 1px solid var(--border-color);
  position: relative;
}

.about-icon-box {
  width: 60px;
  height: 60px;
  border-radius: 16px;
  background: var(--primary-light);
  color: var(--primary-color);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 16px;
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.1);
}

:root.dark .about-icon-box {
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.15);
}

.about-app-icon {
  width: 32px;
  height: 32px;
}

.about-name-modern {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-main);
  margin: 0 0 8px;
  cursor: pointer;
  letter-spacing: -0.02em;
  transition: color var(--transition-speed) ease;
}

.about-name-modern:hover {
  color: var(--primary-color);
}

.about-body-modern {
  text-align: center;
  padding: 20px 24px;
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

.about-footer-modern {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 24px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-app);
  box-sizing: border-box;
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
  background: var(--bg-glass);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border-bottom: 1px solid var(--border-color);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.03);
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

.btn-start, .btn-stop {
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1) !important;
  font-weight: 500 !important;
}

.btn-start:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 10px rgba(16, 185, 129, 0.12);
}

.btn-start:active {
  transform: translateY(0) scale(0.97);
}

.btn-stop:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 10px rgba(244, 63, 94, 0.12);
}

.btn-stop:active {
  transform: translateY(0) scale(0.97);
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

</style>
