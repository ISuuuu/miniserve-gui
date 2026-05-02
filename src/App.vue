<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage, ElMessageBox } from "element-plus";
import { Download, VideoPlay, VideoPause, Refresh, InfoFilled } from "@element-plus/icons-vue";
import { getVersion } from "@tauri-apps/api/app";
import { useI18n } from "vue-i18n";
import ConfigPanel from "./components/ConfigPanel.vue";
import StatusCard from "./components/StatusCard.vue";
import LogPanel from "./components/LogPanel.vue";

const { t } = useI18n();

// ============ Types ============

interface EngineStatus {
  exists: boolean;
  version: string | null;
  path: string;
}

interface ServerConfig {
  path: string;
  port: number;
  interfaces: string;
  auth_username: string;
  auth_password: string;
  upload: boolean;
  mkdir: boolean;
  media_controls: boolean;
  color_scheme: string;
  title: string;
  compress: string;
  hidden: boolean;
  thumbnails: boolean;
  random_route: boolean;
  readme: boolean;
  download: boolean;
  webdav: boolean;
}

interface ServerStatus {
  running: boolean;
  pid: number | null;
  url: string | null;
  urls: string[];
  port: number | null;
}

interface QrResponse {
  data: string;
}

// ============ State ============

const engineStatus = ref<EngineStatus | null>(null);
const serverStatus = ref<ServerStatus | null>(null);
const downloading = ref(false);
const progress = ref(0);
const loading = ref(false);
const qrCodes = ref<string[]>([]);
const serverUrls = ref<string[]>([]);
const logs = ref<string[]>([]);
const copySuccessIdx = ref<Set<number>>(new Set());
const hoveredIdx = ref<number | null>(null);
const hoveredFeature = ref("");

const appVersion = ref("");
const aboutVisible = ref(false);
const checkingUpdate = ref(false);

const config = reactive<ServerConfig>({
  path: "",
  port: 8080,
  interfaces: "::",
  auth_username: "",
  auth_password: "",
  upload: false,
  mkdir: false,
  media_controls: false,
  color_scheme: "squirrel",
  title: "miniserve",
  compress: "",
  hidden: false,
  thumbnails: false,
  random_route: false,
  readme: false,
  download: false,
  webdav: false,
});

// ============ Engine Management ============

async function checkEngine() {
  try {
    engineStatus.value = await invoke<EngineStatus>("get_engine_status");
    if (engineStatus.value && !engineStatus.value.exists) {
      ElMessage.info(t('messages.engineNotInstalledInfo'));
    }
  } catch (e) {
    console.error(e);
  }
}

async function downloadEngine() {
  downloading.value = true;
  progress.value = 0;
  ElMessage.info(t('messages.startDownloadEngine'));
  try {
    const result = await invoke<string>("download_engine");
    downloading.value = false;
    ElMessage.success(t('messages.downloadEngineSuccess', { result }));
    await checkEngine();
  } catch (e) {
    downloading.value = false;
    ElMessage.error(t('messages.downloadEngineFailed', { error: e }));
  }
}

// ============ Config ============

async function loadConfig() {
  try {
    const saved = await invoke<ServerConfig>("load_config");
    // 兼容老版本配置：统一升级为双栈监听
    if (saved && saved.interfaces === "0.0.0.0") {
      saved.interfaces = "::";
    }
    Object.assign(config, saved);
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

async function saveConfig() {
  try {
    await invoke("save_config", { config: { ...config } });
  } catch (e) {
    console.error("Save config failed:", e);
  }
}

// ============ Server Control ============

async function startServer() {
  if (!config.path) {
    ElMessage.warning(t('messages.selectFolderFirst'));
    return;
  }
  if (!engineStatus.value?.exists) {
    ElMessage.warning(t('messages.downloadEngineFirst'));
    return;
  }
  loading.value = true;
  addLog(t('messages.startingService'));
  try {
    const status = await invoke<ServerStatus>("start_server", { config: { ...config } });
    serverStatus.value = status;
    addLog(t('messages.startComplete', { status: JSON.stringify(status) }));
    
    const urlsToShow = status.urls && status.urls.length > 0 ? status.urls : (status.url ? [status.url] : []);
    if (urlsToShow.length > 0) {
      addLog(t('messages.serviceStarted', { urls: urlsToShow.join(', ') }));
      ElMessage.success(t('messages.serviceStarted', { urls: '' }));
      serverUrls.value = urlsToShow;
      qrCodes.value = await Promise.all(
        urlsToShow.map(url => generateQr(url))
      );
    }
  } catch (e) {
    addLog(t('messages.startFailed', { error: e }));
    ElMessage.error(t('messages.startFailed', { error: e }));
  } finally {
    loading.value = false;
    addLog(t('messages.loadingReset'));
  }
}

async function stopServer() {
  loading.value = true;
  addLog(t('messages.stoppingService'));
  try {
    await invoke("stop_server");
    serverStatus.value = { running: false, pid: null, url: null, urls: [], port: null };
    qrCodes.value = [];
    serverUrls.value = [];
    addLog(t('messages.serviceStopped'));
    ElMessage.info(t('messages.serviceStopped'));
  } catch (e) {
    ElMessage.error(t('messages.stopFailed', { error: e }));
  } finally {
    loading.value = false;
  }
}

// ============ QR Code ============

async function generateQr(url: string): Promise<string> {
  try {
    const resp = await invoke<QrResponse>("generate_qr", { data: url });
    return resp.data;
  } catch (e) {
    console.error("QR generation failed:", e);
    return "";
  }
}

async function copyUrl(url?: string, idx?: number) {
  const urlToCopy = url || serverStatus.value?.url || "";
  if (!urlToCopy) return;
  try {
    await navigator.clipboard.writeText(urlToCopy);
    if (idx !== undefined) {
      copySuccessIdx.value = new Set([...copySuccessIdx.value, idx]);
      setTimeout(() => {
        const next = new Set(copySuccessIdx.value);
        next.delete(idx);
        copySuccessIdx.value = next;
      }, 2000);
    }
    ElMessage.success(t('messages.linkCopied'));
  } catch {
    ElMessage.error(t('messages.copyFailed'));
  }
}

async function openUrl(url: string) {
  try {
    const { openUrl: tauriOpenUrl } = await import('@tauri-apps/plugin-opener');
    await tauriOpenUrl(url);
  } catch (e) {
    console.error("Failed to open URL:", e);
    ElMessage.error(t('messages.openUrlFailed', { error: e }));
  }
}

// ============ Path Selection ============

async function selectPath() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      config.path = selected as string;
    }
  } catch (e) {
    const dir = window.prompt("请输入文件夹路径:");
    if (dir) config.path = dir;
  }
}

// ============ Logs ============

function addLog(msg: string) {
  logs.value.push(msg);
  if (logs.value.length > 200) logs.value.shift();
}

function clearLogs() {
  logs.value = [];
}

// ============ Auto Save ============

watch(() => config.upload, (val) => {
  if (!val) config.mkdir = false;
});

let saveTimeout: ReturnType<typeof setTimeout> | null = null;
watch(
  config,
  () => {
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
      saveConfig();
    }, 500);
  },
  { deep: true }
);

onUnmounted(() => {
  if (saveTimeout) {
    clearTimeout(saveTimeout);
    saveTimeout = null;
  }
});

// ============ App Update ============

interface UpdaterConfig {
  endpoints: string[];
  proxy: string | null;
}

async function checkForUpdates() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  addLog(t('update.checking'));

  try {
    const updaterConfig = await invoke<UpdaterConfig>("get_updater_config");
    const originalUrl = updaterConfig.endpoints[0] || "";
    const proxyPrefix = updaterConfig.proxy || "";
    const proxyUrl = proxyPrefix ? `${proxyPrefix}${originalUrl}` : "";

    const { check } = await import('@tauri-apps/plugin-updater');
    
    let update = null;
    try {
      update = await Promise.race([
        check(),
        new Promise<null>((_, reject) => setTimeout(() => reject(new Error("Timeout")), 5000))
      ]);
    } catch (e) {
      if (!proxyUrl) throw e;
      
      addLog(t('update.directConnectTimeout', { proxy: proxyUrl }));
      const resp = await fetch(proxyUrl);
      if (!resp.ok) throw new Error(`代理响应异常: ${resp.status}`);
      const updateJson = await resp.json();

      const currentVersion = appVersion.value.replace(/^v/, '');
      const latestVersion = (updateJson.version || '').replace(/^v/, '');
      
      if (latestVersion && latestVersion !== currentVersion) {
        addLog(t('update.newVersion', { version: latestVersion, current: currentVersion }));
        const platform = `${getPlatform()}-${getArch()}`;
        const platformInfo = updateJson.platforms?.[platform];
        if (!platformInfo) throw new Error(t('update.platformNotAvailable', { platform }));

        ElMessage.success(t('update.downloading', { version: latestVersion }));
        await invoke('download_and_install_update', {
          url: platformInfo.url,
          signature: platformInfo.signature,
          version: latestVersion,
        });
        const { relaunch } = await import('@tauri-apps/plugin-process');
        await relaunch();
        return;
      }
      update = null;
    }

    if (update) {
      const packageType = await invoke<string>("get_package_type");
      if (packageType === "deb" || packageType === "portable") {
        addLog(t('update.debOrPortableNotSupported', { version: update.version, packageType }));
        ElMessageBox.confirm(
          t('update.debOrPortableNotSupported', { 
            version: update.version, 
            packageType: packageType === 'deb' ? 'DEB安装版' : 'Windows便携版' 
          }),
          '发现更新',
          {
            confirmButtonText: t('update.goToDownload'),
            cancelButtonText: t('update.later'),
            type: 'info'
          }
        ).then(() => {
          const releaseUrl = t('update.releasePage');
          openUrl(releaseUrl);
        }).catch(() => {});
        return;
      }
      await installUpdate(update);
    } else {
      ElMessage.info(t('update.alreadyLatest'));
    }
  } catch (e: any) {
    addLog(t('update.checkFailed', { error: e }));
    ElMessage.error(t('update.checkFailed', { error: e.message || e }));
  } finally {
    checkingUpdate.value = false;
  }
}

function getPlatform(): string {
  const platform = navigator.platform.toLowerCase();
  if (platform.includes('win')) return 'windows';
  if (platform.includes('mac')) return 'darwin';
  return 'linux';
}

function getArch(): string {
  // @ts-ignore userAgentData is not in all TS lib versions
  const uaData = navigator.userAgentData;
  if (uaData?.platform) {
    const arch = uaData.architecture || '';
    if (arch.includes('arm') || arch.includes('aarch64')) return 'aarch64';
  }
  return 'x86_64';
}

async function installUpdate(update: any) {
  let installDir = "";
  try {
    installDir = await invoke("get_install_dir");
  } catch (e) {
    console.warn("无法获取安装目录:", e);
  }
  const installerArgs = installDir ? ['/S', `/D=${installDir}`] : undefined;

  ElMessage.success(t('update.downloading', { version: update.version }));
  await update.downloadAndInstall((event: any) => {
    switch (event.event) {
      case 'Started':
        addLog(t('update.downloadStarted', { size: event.data.contentLength }));
        break;
      case 'Progress':
        addLog(t('update.downloadProgress', { size: event.data.chunkLength }));
        break;
      case 'Finished':
        addLog(t('update.downloadFinished'));
        break;
    }
  }, { installerArgs });
  ElMessage.success(t('update.updateComplete'));
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
}

// ============ Lifecycle ============

onMounted(async () => {
  setTimeout(async () => {
    try {
      await invoke("show_window_command");
    } catch (e) {
      console.error("Failed to show window:", e);
    }
  }, 100);

  try {
    appVersion.value = await getVersion();
  } catch (e) {
    console.warn("无法获取 Tauri 版本", e);
  }

  await checkEngine();
  await loadConfig();

  await listen<number>("download-progress", (event) => {
    progress.value = event.payload;
  });

  await listen("server-started", (event) => {
    addLog("Server event: " + JSON.stringify(event.payload));
  });

  await listen<string>("server-log", (event) => {
    addLog(event.payload);
  });
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
            @click="startServer" 
            :loading="loading"
          >
            {{ serverStatus?.running ? t('header.restart') : t('header.start') }}
          </el-button>
          <el-button
            v-if="serverStatus?.running"
            type="danger"
            :icon="VideoPause"
            @click="stopServer"
            :loading="loading"
          >
            {{ t('header.stop') }}
          </el-button>
        </div>
      </div>
      <div class="header-actions">
        <el-tag v-if="engineStatus?.exists" type="success" size="small">
          {{ t('header.engineReady') }} {{ engineStatus.version ? `(${engineStatus.version})` : "" }}
        </el-tag>
        <el-tag v-else type="warning" size="small">{{ t('header.engineNotInstalled') }}</el-tag>
        <el-button
          v-if="!engineStatus?.exists"
          type="primary"
          size="small"
          :icon="Download"
          @click="downloadEngine"
          :loading="downloading"
          :disabled="serverStatus?.running"
        >
          {{ downloading ? t('header.downloading', { progress: progress.toFixed(0) }) : t('header.downloadEngine') }}
        </el-button>
        <el-button
          v-else
          type="info"
          size="small"
          :icon="Refresh"
          @click="downloadEngine"
          :loading="downloading"
          :disabled="serverStatus?.running"
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
            @click="checkForUpdates" 
            :loading="checkingUpdate"
          >{{ t('about.checkUpdate') }}</el-button>
          <el-button @click="aboutVisible = false">{{ t('about.close') }}</el-button>
        </div>
      </template>
    </el-dialog>

    <el-progress
      v-if="downloading"
      :percentage="progress"
      :format="(p: number) => `${p.toFixed(1)}%`"
      class="download-progress"
    />

    <div class="main-layout">
      <!-- Config Panel -->
      <ConfigPanel 
        :config="config" 
        :hovered-feature="hoveredFeature"
        @select-path="selectPath"
        @update:hovered-feature="hoveredFeature = $event"
      />

      <!-- Right Panel: QR + Logs -->
      <main class="right-panel">
        <!-- Server Status Card -->
        <StatusCard
          :server-status="serverStatus"
          :server-urls="serverUrls"
          :qr-codes="qrCodes"
          :copy-success-idx="copySuccessIdx"
          :hovered-idx="hoveredIdx"
          @copy-url="copyUrl"
          @open-url="openUrl"
          @hover-url="hoveredIdx = $event"
        />

        <!-- Log Panel -->
        <LogPanel
          :logs="logs"
          @clear-logs="clearLogs"
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
