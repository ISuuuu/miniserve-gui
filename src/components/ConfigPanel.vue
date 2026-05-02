<script setup lang="ts">
import { FolderOpened, Setting, Lock, Picture, MagicStick } from "@element-plus/icons-vue";
import { useI18n } from "vue-i18n";

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

defineProps<{
  config: ServerConfig;
}>();

const emit = defineEmits<{
  selectPath: [];
}>();

const { t } = useI18n();

const colorSchemes = [
  { label: t('colorSchemes.squirrel'), value: "squirrel" },
  { label: t('colorSchemes.archlinux'), value: "archlinux" },
  { label: t('colorSchemes.zenburn'), value: "zenburn" },
  { label: t('colorSchemes.monokai'), value: "monokai" },
];

const interfaceOptions = [
  { label: t('config.interfaceAll'), value: "::" },
  { label: t('config.interfaceLocal'), value: "127.0.0.1" },
];

const featureDescriptions: Record<string, string> = {
  random_route: t('features.randomRoute'),
  webdav: t('features.webdav'),
  download: t('features.download'),
  readme: t('features.readme'),
  hidden: t('features.hidden'),
};

const hoveredFeature = defineModel<string>("hoveredFeature", { default: "" });

function selectPath() {
  emit("selectPath");
}
</script>

<template>
  <aside class="config-panel">
    <el-form label-width="90" size="small">
      <div class="section-title"><el-icon><Setting /></el-icon> {{ t('config.basic') }}</div>
      <el-form-item :label="t('config.sharePath')">
        <div class="path-row">
          <el-input v-model="config.path" :placeholder="t('config.sharePathPlaceholder')" readonly />
          <el-button type="primary" :icon="FolderOpened" @click="selectPath" />
        </div>
      </el-form-item>

      <el-form-item :label="t('config.port')">
        <el-input-number v-model="config.port" :min="1" :max="65535" />
      </el-form-item>

      <el-form-item :label="t('config.interface')">
        <el-select v-model="config.interfaces">
          <el-option
            v-for="opt in interfaceOptions"
            :key="opt.value"
            :label="opt.label"
            :value="opt.value"
          />
        </el-select>
      </el-form-item>

      <div class="section-title"><el-icon><Lock /></el-icon> {{ t('config.security') }}</div>
      <el-form-item :label="t('config.username')">
        <el-input v-model="config.auth_username" :placeholder="t('config.usernamePlaceholder')" />
      </el-form-item>

      <el-form-item :label="t('config.password')">
        <el-input
          v-model="config.auth_password"
          type="password"
          :placeholder="t('config.passwordPlaceholder')"
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
          <span class="toggle-dot" />{{ t('config.upload') }}
        </button>
        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.mkdir }"
          v-if="config.upload"
          @click="config.mkdir = !config.mkdir"
        >
          <span class="toggle-dot" />{{ t('config.mkdir') }}
        </button>
      </div>

      <div class="section-title"><el-icon><Picture /></el-icon> {{ t('config.display') }}</div>
      <el-form-item :label="t('config.colorScheme')">
        <el-select v-model="config.color_scheme" :placeholder="t('config.colorSchemePlaceholder')">
          <el-option
            v-for="cs in colorSchemes"
            :key="cs.value"
            :label="cs.label"
            :value="cs.value"
          />
        </el-select>
      </el-form-item>

      <el-form-item :label="t('config.title')">
        <el-input v-model="config.title" />
      </el-form-item>

      <div class="section-title"><el-icon><MagicStick /></el-icon> {{ t('config.advanced') }}</div>
      <div class="two-col">
        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.random_route }"
          @click="config.random_route = !config.random_route"
          @mouseenter="hoveredFeature = 'random_route'"
          @mouseleave="hoveredFeature = ''"
        >
          <span class="toggle-dot" />{{ t('config.randomRoute') }}
        </button>

        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.webdav }"
          @click="config.webdav = !config.webdav"
          @mouseenter="hoveredFeature = 'webdav'"
          @mouseleave="hoveredFeature = ''"
        >
          <span class="toggle-dot" />{{ t('config.webdav') }}
        </button>

        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.download }"
          @click="config.download = !config.download"
          @mouseenter="hoveredFeature = 'download'"
          @mouseleave="hoveredFeature = ''"
        >
          <span class="toggle-dot" />{{ t('config.download') }}
        </button>

        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.readme }"
          @click="config.readme = !config.readme"
          @mouseenter="hoveredFeature = 'readme'"
          @mouseleave="hoveredFeature = ''"
        >
          <span class="toggle-dot" />{{ t('config.readme') }}
        </button>

        <button
          type="button"
          class="toggle-pill"
          :class="{ active: config.hidden }"
          @click="config.hidden = !config.hidden"
          @mouseenter="hoveredFeature = 'hidden'"
          @mouseleave="hoveredFeature = ''"
        >
          <span class="toggle-dot" />{{ t('config.hidden') }}
        </button>
      </div>
    </el-form>

    <div class="feature-hint">
      <span>{{ hoveredFeature ? featureDescriptions[hoveredFeature] || '' : '' }}</span>
    </div>
  </aside>
</template>

<style scoped>
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
