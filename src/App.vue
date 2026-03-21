<script setup lang="ts">
import { ref, reactive, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { FolderOpened, Download, VideoPlay, VideoPause, Refresh, DocumentCopy } from "@element-plus/icons-vue";

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
const copySuccess = ref(false);
const hoveredIdx = ref<number | null>(null);

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
});

const colorSchemes = [
  { label: "🐿️ 松鼠 (squirrel)", value: "squirrel" },
  { label: "🐧 Arch Linux (archlinux)", value: "archlinux" },
  { label: "🎋 禅意 (zenburn)", value: "zenburn" },
  { label: "🍈 物语 (monokai)", value: "monokai" },
];



const interfaceOptions = [
  { label: "所有网卡 (0.0.0.0)", value: "0.0.0.0" },
  { label: "本地回环 (127.0.0.1)", value: "127.0.0.1" },
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
      ElMessage.success("服务已启动: " + urlsToShow.join(', '));
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

async function copyUrl(url?: string) {
  const urlToCopy = url || serverStatus.value?.url || "";
  if (!urlToCopy) return;
  try {
    await navigator.clipboard.writeText(urlToCopy);
    copySuccess.value = true;
    setTimeout(() => (copySuccess.value = false), 2000);
    ElMessage.success("链接已复制");
  } catch {
    ElMessage.error("复制失败");
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
  const ts = new Date().toLocaleTimeString();
  logs.value.push(`[${ts}] ${msg}`);
  if (logs.value.length > 200) logs.value.shift();
}

// ============ Auto Save ============

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

// ============ Lifecycle ============

onMounted(async () => {
  await checkEngine();
  await loadConfig();
  await checkServerStatus();

  await listen<number>("download-progress", (event) => {
    progress.value = event.payload;
  });

  await listen("server-started", (event) => {
    addLog("Server event: " + JSON.stringify(event.payload));
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
        >
          更新引擎
        </el-button>
      </div>
    </header>

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
          <div class="section-title">📂 基础配置</div>
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

          <div class="section-title">🔐 安全控制</div>
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

          <el-form-item>
            <el-switch v-model="config.upload" /> &nbsp; 上传文件
            <el-switch v-model="config.mkdir" v-if="config.upload" style="margin-left: 16px" /> &nbsp; <span v-if="config.upload">创建目录</span>
          </el-form-item>

          <!-- <el-form-item v-if="config.upload">
            <el-switch v-model="config.media_controls" /> &nbsp; 允许媒体操作
          </el-form-item> -->

          <div class="section-title">🎨 界面展示</div>
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

          <div class="section-title">⚙️ 高级进阶</div>
          <div class="two-col">
            <el-form-item>
              <div class="switch-row">
                <el-switch v-model="config.hidden" />
                <span>显示隐藏文件</span>
              </div>
            </el-form-item>

            <el-form-item>
              <div class="switch-row">
                <el-switch v-model="config.random_route" />
                <span>随机路径</span>
              </div>
            </el-form-item>

            <el-form-item>
              <div class="switch-row">
                <el-switch v-model="config.readme" />
                <span>README渲染</span>
              </div>
            </el-form-item>

            <el-form-item>
              <div class="switch-row">
                <el-switch v-model="config.download" />
                <span>一键打包下载</span>
              </div>
            </el-form-item>
          </div>

          <!-- <el-form-item>
            <el-switch v-model="config.thumbnails" /> &nbsp; 生成缩略图
          </el-form-item> -->
        </el-form>


      </aside>

      <!-- Right Panel: QR + Logs -->
      <main class="right-panel">
        <!-- Server Status Card -->
        <el-card v-if="serverStatus?.running" class="status-card" shadow="hover">
          <template #header>
            <div class="card-header">
              <span>🚀 服务运行中</span>
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
                <el-link type="primary" :href="url" target="_blank">
                  {{ url }}
                </el-link>
                <el-button type="primary" size="small" text @click="copyUrl(url)">
                  <el-icon><DocumentCopy /></el-icon>
                  {{ copySuccess ? "已复制" : "复制" }}
                </el-button>
              </div>
            </div>
            <div class="qr-column">
              <div v-if="hoveredIdx !== null && qrCodes[hoveredIdx]" class="qr-display">
                <img :src="qrCodes[hoveredIdx]" alt="QR" class="qr-img" />
              </div>
              <div v-else class="qr-placeholder">
                鼠标悬停地址查看二维码
              </div>
            </div>
          </div>
        </el-card>

        <el-card v-else class="status-card" shadow="hover">
          <div class="idle-state">
            <p>⬜ 服务未运行</p>
            <p class="hint">配置好参数后点击「启动服务」</p>
          </div>
        </el-card>

        <!-- Log Panel -->
        <el-card class="log-card" shadow="hover">
          <template #header>
            <div class="card-header">
              <span>📋 运行日志</span>
              <el-button text size="small" @click="logs = []">清空</el-button>
            </div>
          </template>
          <div class="log-box">
            <p v-for="(log, i) in logs" :key="i" class="log-line">{{ log }}</p>
            <p v-if="logs.length === 0" class="log-empty">暂无日志</p>
          </div>
        </el-card>
      </main>
    </div>
  </div>
</template>

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
  padding: 12px 20px;
  background: #fff;
  border-bottom: 1px solid #e4e7ed;
}

.app-header h2 {
  margin: 0;
  font-size: 18px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
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
  width: 320px;
  min-width: 280px;
  background: #fff;
  padding: 16px;
  overflow-y: auto;
  border-right: 1px solid #e4e7ed;
}

.path-row {
  display: flex;
  gap: 8px;
  width: 100%;
}



.path-row .el-input {
  flex: 1;
}

.panel-actions {
  display: flex;
  gap: 8px;
  margin-top: 20px;
  flex-wrap: wrap;
}

.two-col {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.two-col .el-form-item {
  margin-bottom: 6px;
}

/* 将无标签的表单项内容区右移，与带标签的控件对齐 */
.two-col .el-form-item .el-form-item__content {
  margin-left: 70px; /* 向左移动一些以更接近配色方案位置 */
  padding: 0;
}

.switch-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.right-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
  min-height: 0;
}

.status-card {
  flex-shrink: 0;
  flex: 0 1 220px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
}

.url-layout {
  display: flex;
  gap: 16px;
  min-height: 120px;
}

.url-column {
  flex: 0 0 55%;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.url-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: #f5f7fa;
  border-radius: 6px;
  gap: 8px;
  font-size: 13px;
  transition: all 0.2s;
  width: 100%;
}

.url-item .el-link {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.url-item.active {
  background: #ecf5ff;
  border: 1px solid #409eff;
}

.qr-column {
  width: 120px;
  min-height: 120px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #fafafa;
  border-radius: 8px;
}

.qr-display {
  text-align: center;
}

.qr-display .qr-img {
  width: 120px;
  height: 120px;
}

.qr-hint {
  margin-top: 8px;
  font-size: 12px;
  color: #606266;
  word-break: break-all;
}

.qr-placeholder {
  color: #909399;
  font-size: 13px;
  text-align: center;
  padding: 16px;
}

.idle-state {
  text-align: center;
  padding: 20px;
  color: #909399;
}

.idle-state .hint {
  font-size: 13px;
  margin-top: 8px;
}

.log-card {
  flex: 1 1 350px;
  display: flex;
  flex-direction: column;
}

.log-box {
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 16px;
  border-radius: 4px;
  font-family: "Consolas", "Monaco", monospace;
  font-size: 12px;
  line-height: 1.6;
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
</style>
