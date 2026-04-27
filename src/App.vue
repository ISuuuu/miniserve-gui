<script setup lang="ts">
import { ref, reactive, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage, ElMessageBox } from "element-plus";
import { FolderOpened, Download, VideoPlay, VideoPause, Refresh, DocumentCopy, InfoFilled, Setting, Lock, Picture, MagicStick, Files, Cpu } from "@element-plus/icons-vue";
import { getVersion } from "@tauri-apps/api/app";

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
  // hide_icons: boolean;
  // spa: boolean;
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
const qrCodeUrl = ref("");
const qrCodes = ref<string[]>([]);
const serverUrls = ref<string[]>([]);
const logs = ref<string[]>([]);
const logBoxRef = ref<HTMLElement | null>(null);
const copySuccessIdx = ref<Set<number>>(new Set());
const hoveredIdx = ref<number | null>(null);
const hoveredFeature = ref("");

const featureDescriptions: Record<string, string> = {
  random_route: "为服务器路径添加随机后缀，防止被他人扫描访问",
  webdav: "启用 WebDAV 协议，支持通过 WebDAV 客户端远程管理文件",
  download: "允许访客将目录打包为 ZIP 文件下载",
  readme: "自动识别并渲染目录中的 README 文件作为首页",
  hidden: "显示以 . 开头的隐藏文件和文件夹",
};

const appVersion = ref("");
const aboutVisible = ref(false);
const checkingUpdate = ref(false);

const config = reactive<ServerConfig>({
  path: "",
  port: 8080,
  interfaces: "0.0.0.0",
  auth_username: "",
  auth_password: "",
  upload: false,
  mkdir: false,
  media_controls: false,
  color_scheme: "squirrel",
  title: "miniserve",
  // hide_icons: false,
  // spa: false,
  compress: "",
  hidden: false,
  thumbnails: false,
  random_route: false,
  readme: false,
  download: false,
  webdav: false,
});

const colorSchemes = [
  { label: "🐿️ 松鼠 (squirrel)", value: "squirrel" },
  { label: "🐧 Arch Linux (archlinux)", value: "archlinux" },
  { label: "🎋 禅意 (zenburn)", value: "zenburn" },
  { label: "🍈 物语 (monokai)", value: "monokai" },
];



const interfaceOptions = [
  { label: "所有网卡", value: "::" },
  { label: "仅本机 (127.0.0.1)", value: "127.0.0.1" },
];

// ============ Engine Management ============

async function checkEngine() {
  try {
    engineStatus.value = await invoke<EngineStatus>("get_engine_status");
    if (engineStatus.value && !engineStatus.value.exists) {
      ElMessage.info("引擎未安装，点击下载");
    }
  } catch (e) {
    console.error(e);
  }
}

async function downloadEngine() {
  downloading.value = true;
  progress.value = 0;
  ElMessage.info("开始下载引擎...");
  try {
    const result = await invoke<string>("download_engine");
    downloading.value = false;
    ElMessage.success("引擎下载成功：" + result);
    await checkEngine();
  } catch (e) {
    downloading.value = false;
    ElMessage.error("下载失败: " + e);
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

// 保存配置（自动保存使用）
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
    ElMessage.warning("请先选择要分享的文件夹路径");
    return;
  }
  if (!engineStatus.value?.exists) {
    ElMessage.warning("请先下载引擎");
    return;
  }
  loading.value = true;
  addLog("正在启动服务...");
  try {
    const status = await invoke<ServerStatus>("start_server", { config: { ...config } });
    serverStatus.value = status;
    addLog("启动完成: " + JSON.stringify(status));
    
    // 显示所有 URL
    const urlsToShow = status.urls && status.urls.length > 0 ? status.urls : (status.url ? [status.url] : []);
    if (urlsToShow.length > 0) {
      addLog("服务已启动: " + urlsToShow.join(', '));
      ElMessage.success("服务已启动:");
      serverUrls.value = urlsToShow;
      // 为每个 URL 生成二维码
      qrCodes.value = await Promise.all(
        urlsToShow.map(url => generateQr(url))
      );
    }
  } catch (e) {
    addLog("启动失败: " + e);
    ElMessage.error("启动失败: " + e);
  } finally {
    loading.value = false;
    addLog("loading 已重置");
  }
}

async function stopServer() {
  loading.value = true;
  addLog("正在停止服务...");
  try {
    await invoke("stop_server");
    serverStatus.value = { running: false, pid: null, url: null, urls: [], port: null };
    qrCodeUrl.value = "";
    qrCodes.value = [];
    serverUrls.value = [];
    addLog("服务已停止");
    ElMessage.info("服务已停止");
  } catch (e) {
    ElMessage.error("停止失败: " + e);
  } finally {
    loading.value = false;
  }
}

async function checkServerStatus() {
  try {
    serverStatus.value = await invoke<ServerStatus>("get_server_status");
    if (serverStatus.value?.url) {
      await generateQr(serverStatus.value.url);
    }
  } catch (e) {
    console.error(e);
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
    ElMessage.success("链接已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

async function openUrl(url: string) {
  try {
    const { openUrl: tauriOpenUrl } = await import('@tauri-apps/plugin-opener');
    await tauriOpenUrl(url);
  } catch (e) {
    console.error("Failed to open URL:", e);
    ElMessage.error("调用浏览器失败: " + e);
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
    // Fallback: use prompt
    const dir = window.prompt("请输入文件夹路径:");
    if (dir) config.path = dir;
  }
}

// ============ Logs ============

function addLog(msg: string) {
  logs.value.push(msg);
  if (logs.value.length > 200) logs.value.shift();
  setTimeout(() => {
    if (logBoxRef.value) {
      const lastLog = logBoxRef.value.querySelector('.log-line:last-child');
      if (lastLog) {
        lastLog.scrollIntoView({ behavior: 'smooth' });
      }
    }
  }, 50);
}

// ============ Auto Save ============

// 上传文件关闭时，自动取消创建目录
watch(() => config.upload, (val) => {
  if (!val) config.mkdir = false;
});

// 自动保存配置（防抖）
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

// ============ App Update ============

interface UpdaterConfig {
  endpoints: string[];
  proxy: string | null;
}

async function checkForUpdates() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  addLog("正在检查软件更新...");

  try {
    const config = await invoke<UpdaterConfig>("get_updater_config");
    const originalUrl = config.endpoints[0] || "";
    const proxyPrefix = config.proxy || "";
    const proxyUrl = proxyPrefix ? `${proxyPrefix}${originalUrl}` : "";

    const { check } = await import('@tauri-apps/plugin-updater');
    
    let update = null;
    try {
      // 尝试直连，5秒内无响应则强制超时进入代理逻辑
      update = await Promise.race([
        check(),
        new Promise<null>((_, reject) => setTimeout(() => reject(new Error("Timeout")), 5000))
      ]);
    } catch (e) {
      if (!proxyUrl) throw e;
      
      addLog("直连检查超时或失败，尝试使用代理: " + proxyUrl);
      const resp = await fetch(proxyUrl);
      if (!resp.ok) throw new Error(`代理响应异常: ${resp.status}`);
      const updateJson = await resp.json();

      const currentVersion = appVersion.value.replace(/^v/, '');
      const latestVersion = (updateJson.version || '').replace(/^v/, '');
      
      if (latestVersion && latestVersion !== currentVersion) {
        addLog(`发现新版本 v${latestVersion} (当前: v${currentVersion})`);
        const platform = `${getPlatform()}-${getArch()}`;
        const platformInfo = updateJson.platforms?.[platform];
        if (!platformInfo) throw new Error(`当前平台 ${platform} 无可用更新`);

        ElMessage.success(`发现新版本 v${latestVersion}，正在下载并安装...`);
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
        addLog(`发现新版本 v${update.version}，当前包类型 [${packageType}] 不支持自动更新，引导至 Release 页面`);
        ElMessageBox.confirm(
          `发现新版本 v${update.version}，当前 [${packageType === 'deb' ? 'DEB安装版' : 'Windows便携版'}] 不支持自动更新。是否前往下载页面手动更新？`,
          '发现更新',
          {
            confirmButtonText: '前往下载',
            cancelButtonText: '稍后再说',
            type: 'info'
          }
        ).then(() => {
          const releaseUrl = "https://github.com/ISuuuu/miniserve-gui/releases/latest";
          openUrl(releaseUrl);
        }).catch(() => {});
        return;
      }
      await installUpdate(update);
    } else {
      ElMessage.info("已是最新版本");
    }
  } catch (e: any) {
    addLog("检查更新失败: " + e);
    ElMessage.error("检查更新失败: " + (e.message || e));
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
  // Fallback: assume x86_64
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

  ElMessage.success(`发现新版本 v${update.version}，正在下载并安装...`);
  await update.downloadAndInstall((event: any) => {
    switch (event.event) {
      case 'Started':
        addLog(`开始下载更新 (大小: ${event.data.contentLength} 字节)`);
        break;
      case 'Progress':
        addLog(`已下载: ${event.data.chunkLength} 字节`);
        break;
      case 'Finished':
        addLog('下载完成');
        break;
    }
  }, { installerArgs });
  ElMessage.success('更新完成，即将重启...');
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
}

// ============ Lifecycle ============

onMounted(async () => {
  // 尽早显示窗口，解决初始白屏闪烁问题，防止后续 await 阻塞导致窗口一直隐藏
  // 调用后端特制的 show_main_window，解决 Windows 焦点抢占问题
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
  await checkServerStatus();

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
            {{ serverStatus?.running ? "重启" : "启动" }}
          </el-button>
          <el-button
            v-if="serverStatus?.running"
            type="danger"
            :icon="VideoPause"
            @click="stopServer"
            :loading="loading"
          >
            停止
          </el-button>
        </div>
      </div>
      <div class="header-actions">
        <el-tag v-if="engineStatus?.exists" type="success" size="small">
          ✅ 引擎已就绪 {{ engineStatus.version ? `(${engineStatus.version})` : "" }}
        </el-tag>
        <el-tag v-else type="warning" size="small">⚠️ 引擎未安装</el-tag>
        <el-button
          v-if="!engineStatus?.exists"
          type="primary"
          size="small"
          :icon="Download"
          @click="downloadEngine"
          :loading="downloading"
          :disabled="serverStatus?.running"
        >
          {{ downloading ? `下载中 ${progress.toFixed(0)}%` : "下载引擎" }}
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
          更新引擎
        </el-button>

        <el-button
          circle
          size="small"
          :icon="InfoFilled"
          @click="aboutVisible = true"
          title="关于软件"
          style="margin-left: 10px;"
        />
      </div>
    </header>

    <!-- 关于软件 Dialog -->
    <el-dialog v-model="aboutVisible" title="关于" width="400px" align-center>
      <div style="text-align: center; margin-bottom: 20px;">
        <h3 style="margin-bottom: 5px; cursor: pointer; color: #409EFF;" @click="openUrl('https://github.com/ISuuuu/miniserve-gui')">miniserve-gui</h3>
        <el-tag type="info" size="small" style="margin-bottom: 15px;">v{{ appVersion || '未知版本' }}</el-tag>
        <p style="font-size: 13px; color: #606266; line-height: 1.6;">
          一个轻量级的跨平台文件分享工具。<br/>
          基于 Tauri 和 <a href="#" @click.prevent="openUrl('https://github.com/svenstaro/miniserve')" style="color: #409EFF; text-decoration: none;">svenstaro/miniserve</a> 构建。
        </p>
      </div>
      <template #footer>
        <div style="display: flex; justify-content: space-between; align-items: center;">
          <el-button 
            type="primary" 
            @click="checkForUpdates" 
            :loading="checkingUpdate"
          >检查软件更新</el-button>
          <el-button @click="aboutVisible = false">关闭</el-button>
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
      <aside class="config-panel">
        <el-form label-width="90" size="small">
          <div class="section-title"><el-icon><Setting /></el-icon> 基础配置</div>
          <el-form-item label="分享路径">
            <div class="path-row">
              <el-input v-model="config.path" placeholder="选择或输入文件夹路径" readonly />
              <el-button type="primary" :icon="FolderOpened" @click="selectPath" />
            </div>
          </el-form-item>

          <el-form-item label="端口">
            <el-input-number v-model="config.port" :min="1" :max="65535" />
          </el-form-item>

          <el-form-item label="绑定网卡">
            <el-select v-model="config.interfaces">
              <el-option
                v-for="opt in interfaceOptions"
                :key="opt.value"
                :label="opt.label"
                :value="opt.value"
              />
            </el-select>
          </el-form-item>

          <div class="section-title"><el-icon><Lock /></el-icon> 安全控制</div>
          <el-form-item label="用户名">
            <el-input v-model="config.auth_username" placeholder="留空则不验证" />
          </el-form-item>

          <el-form-item label="密码">
            <el-input
              v-model="config.auth_password"
              type="password"
              placeholder="留空则不验证"
              show-password
            />
          </el-form-item>

          <div class="toggle-row">
            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.upload }"
              @click="config.upload = !config.upload"
            >
              <span class="toggle-dot" />上传文件
            </button>
            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.mkdir }"
              v-if="config.upload"
              @click="config.mkdir = !config.mkdir"
            >
              <span class="toggle-dot" />创建目录
            </button>
          </div>

          <!-- <el-form-item v-if="config.upload">
            <el-switch v-model="config.media_controls" /> &nbsp; 允许媒体操作
          </el-form-item> -->

          <div class="section-title"><el-icon><Picture /></el-icon> 界面展示</div>
          <el-form-item label="配色方案">
            <el-select v-model="config.color_scheme" placeholder="选择配色方案">
              <el-option
                v-for="cs in colorSchemes"
                :key="cs.value"
                :label="cs.label"
                :value="cs.value"
              />
            </el-select>
          </el-form-item>

          <el-form-item label="网页标题">
            <el-input v-model="config.title" />
          </el-form-item>

          <!-- <el-form-item>
            <el-switch v-model="config.hide_icons" /> &nbsp; 隐藏文件图标
          </el-form-item> -->

          <!-- <el-form-item label="压缩算法">
            <el-select v-model="config.compress" clearable>
              <el-option
                v-for="opt in compressOptions"
                :key="opt.value"
                :label="opt.label"
                :value="opt.value"
              />
            </el-select>
          </el-form-item> -->

          <div class="section-title"><el-icon><MagicStick /></el-icon> 高级进阶</div>
          <div class="two-col">
            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.random_route }"
              @click="config.random_route = !config.random_route"
              @mouseenter="hoveredFeature = 'random_route'"
              @mouseleave="hoveredFeature = ''"
            >
              <span class="toggle-dot" />随机路径
            </button>

            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.webdav }"
              @click="config.webdav = !config.webdav"
              @mouseenter="hoveredFeature = 'webdav'"
              @mouseleave="hoveredFeature = ''"
            >
              <span class="toggle-dot" />WebDAV
            </button>

            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.download }"
              @click="config.download = !config.download"
              @mouseenter="hoveredFeature = 'download'"
              @mouseleave="hoveredFeature = ''"
            >
              <span class="toggle-dot" />打包下载
            </button>

            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.readme }"
              @click="config.readme = !config.readme"
              @mouseenter="hoveredFeature = 'readme'"
              @mouseleave="hoveredFeature = ''"
            >
              <span class="toggle-dot" />README
            </button>

            <button
              type="button"
              class="toggle-pill"
              :class="{ active: config.hidden }"
              @click="config.hidden = !config.hidden"
              @mouseenter="hoveredFeature = 'hidden'"
              @mouseleave="hoveredFeature = ''"
            >
              <span class="toggle-dot" />显示点文件
            </button>
          </div>

          <!-- <el-form-item>
            <el-switch v-model="config.thumbnails" /> &nbsp; 生成缩略图
          </el-form-item> -->
        </el-form>

        <div class="feature-hint">
          <span>{{ hoveredFeature ? featureDescriptions[hoveredFeature] || '' : '' }}</span>
        </div>
      </aside>

      <!-- Right Panel: QR + Logs -->
      <main class="right-panel">
        <!-- Server Status Card -->
        <el-card v-if="serverStatus?.running" class="status-card" shadow="hover">
          <template #header>
            <div class="card-header">
              <span><el-icon><Cpu /></el-icon> 服务运行中</span>
              <el-tag type="success" size="small">PID {{ serverStatus.pid }}</el-tag>
            </div>
          </template>
          <div class="url-layout">
            <div class="url-column">
              <div 
                v-for="(url, idx) in serverUrls" 
                :key="idx" 
                class="url-item"
                :class="{ active: hoveredIdx === idx }"
                @mouseenter="hoveredIdx = idx"
                @mouseleave="hoveredIdx = null"
              >
                <el-link type="primary" :href="url" :underline="false" @click.prevent="openUrl(url)">
                  {{ url }}
                </el-link>
                <el-button type="primary" size="small" text @click="copyUrl(url, idx)">
                  <el-icon><DocumentCopy /></el-icon>
                  {{ copySuccessIdx.has(idx) ? "已复制" : "复制" }}
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
                <span>鼠标悬停地址查看二维码</span>
              </div>
            </div>
          </div>
        </el-card>

        <el-card v-else class="status-card" shadow="hover">
          <div class="idle-state">
            <p><el-icon><Cpu /></el-icon> 服务未运行</p>
            <p class="hint">配置好参数后点击「启动服务」</p>
          </div>
        </el-card>

        <!-- Log Panel -->
        <el-card class="log-card" shadow="hover">
          <template #header>
            <div class="card-header">
              <span><el-icon><Files /></el-icon> 运行日志</span>
              <el-button text size="small" @click="logs = []">清空</el-button>
            </div>
          </template>
          <div ref="logBoxRef" class="log-box">
            <p v-for="(log, i) in logs" :key="i" class="log-line">{{ log }}</p>
            <p v-if="logs.length === 0" class="log-empty">暂无日志</p>
          </div>
        </el-card>
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

.config-panel {
  width: 300px;
  min-width: 280px;
  background: #fff;
  padding: 10px 12px 36px;
  overflow-y: auto;
  border-right: 1px solid #e4e7ed;
  position: relative;
}

.path-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.path-row .el-input {
  flex: 1;
}

.path-row .el-button {
  background: #409EFF;
  border-color: #409EFF;
  transition: all 0.2s ease;
}

.path-row .el-button:hover {
  background: #337ECC;
  border-color: #337ECC;
  transform: translateY(-1px);
}

.toggle-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin: 4px 0 8px;
  padding-left: 80px;
}

.toggle-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 10px;
  border-radius: 6px;
  border: none;
  background: #F4F4F5;
  color: #3F3F46;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  white-space: nowrap;
  outline: none;
}

.toggle-pill:hover {
  background: #E4E4E7;
}

.toggle-pill.active {
  background: #F4F4F5;
  color: #3F3F46;
}

.toggle-pill.active:hover {
  background: #E4E4E7;
}

.toggle-pill.active .toggle-dot {
  background: #409EFF;
}

.toggle-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #A1A1AA;
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.two-col {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 4px 0 8px;
  padding-left: 80px;
}

.panel-actions {
  display: flex;
  gap: 8px;
  margin-top: 20px;
  flex-wrap: wrap;
}

.two-col .el-form-item {
  margin-bottom: 6px;
}

.two-col .el-form-item .el-form-item__content {
  margin-left: 70px;
  padding: 0;
}

.switch-row {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #606266;
  font-size: 13px;
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

.qr-hint {
  margin-top: 8px;
  font-size: 12px;
  color: #606266;
  word-break: break-all;
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

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  color: #409EFF;
  margin: 12px 0 8px;
  padding-bottom: 6px;
  border-bottom: 1px solid #e4e7ed;
}

.section-title .el-icon {
  font-size: 16px;
}

/* 数字输入框内部垂直居中对齐 */
:deep(.el-input-number .el-input__inner) {
  line-height: normal;
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

/* 深色日志区域滚动条 */
.log-container::-webkit-scrollbar-thumb {
  background: #4c4d4f;
}

.log-container::-webkit-scrollbar-thumb:hover {
  background: #6c6d6f;
}

:deep(.el-button--success) {
  background: #67C23A;
  border-color: #67C23A;
}

:deep(.el-button--success:hover) {
  background: #5DAB34;
  border-color: #5DAB34;
}

.feature-hint {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 8px 12px;
  font-size: 11px;
  color: #909399;
  background: #fff;
  border-top: 1px solid #e4e7ed;
  line-height: 1.4;
  z-index: 1;
}
</style>
