<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Download, VideoPlay, VideoPause, Refresh, InfoFilled } from "@element-plus/icons-vue";
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
          >
            {{ serverModule.serverStatus.value?.running ? t('header.restart') : t('header.start') }}
          </el-button>
          <el-button
            v-if="serverModule.serverStatus.value?.running"
            type="danger"
            :icon="VideoPause"
            @click="serverModule.stopServer"
            :loading="serverModule.loading.value"
          >
            {{ t('header.stop') }}
          </el-button>
        </div>
      </div>
      <div class="header-actions">
        <el-tag v-if="engineModule.engineStatus.value?.exists" type="success" size="small">
          {{ t('header.engineReady') }} {{ engineModule.engineStatus.value.version ? `(${engineModule.engineStatus.value.version})` : "" }}
        </el-tag>
        <el-tag v-else type="warning" size="small">{{ t('header.engineNotInstalled') }}</el-tag>
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
          :icon="InfoFilled"
          @click="aboutVisible = true"
          :title="t('header.about')"
          style="margin-left: 10px;"
        />
      </div>
    </header>

    <!-- 关于软件 Dialog -->
    <el-dialog v-model="aboutVisible" :title="t('about.title')" width="400px" align-center>
      <div style="text-align: center; margin-bottom: 20px;">
        <h3 style="margin-bottom: 5px; cursor: pointer; color: #409EFF;" @click="openUrl('https://github.com/ISuuuu/miniserve-gui')">miniserve-gui</h3>
        <el-tag type="info" size="small" style="margin-bottom: 15px;">{{ t('about.version', { version: appVersion || t('about.unknownVersion') }) }}</el-tag>
        <p style="font-size: 13px; color: #606266; line-height: 1.6;">
          {{ t('app.description') }}<br/>
          {{ t('app.basedOn', { link: '' }) }}<a href="#" @click.prevent="openUrl('https://github.com/svenstaro/miniserve')" style="color: #409EFF; text-decoration: none;">svenstaro/miniserve</a>
        </p>
      </div>
      <template #footer>
        <div style="display: flex; justify-content: space-between; align-items: center;">
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
html, body {
  margin: 0;
  padding: 0;
  overflow: hidden;
  height: 100%;
}

/* Webkit 自定义滚动条 - Mac 风格细长圆润 */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: #dcdfe6;
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: #c0c4cc;
}
</style>

<style scoped>
.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #f5f7fa;
  overflow: hidden;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: #fff;
  border-bottom: 1px solid #e4e7ed;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
}

.app-header h2 {
  margin: 0;
  font-size: 18px;
  color: #1E293B;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
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
  overflow-y: auto;
  min-height: 0;
}

:deep(.el-button--success) {
  background: #67C23A;
  border-color: #67C23A;
}

:deep(.el-button--success:hover) {
  background: #5DAB34;
  border-color: #5DAB34;
}
</style>
