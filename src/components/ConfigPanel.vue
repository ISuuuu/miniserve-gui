<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  FolderOpenOutline,
  SettingsOutline,
  LockClosedOutline,
  ImageOutline,
  FlashOutline,
} from "@vicons/ionicons5";
import TogglePill from "./TogglePill.vue";
import type { ServerConfig } from "../types";

const { t } = useI18n();

defineProps<{
  config: ServerConfig;
  hoveredFeature: string;
}>();

const emit = defineEmits<{
  "select-path": [];
  "update:hovered-feature": [feature: string];
}>();

const interfaceOptions = computed(() => [
  { label: t('config.interfaceAll'), value: "::" },
  { label: t('config.interfaceLocal'), value: "127.0.0.1" },
]);

const colorSchemeOptions = computed(() => [
  { label: t('colorSchemes.squirrel'), value: "squirrel" },
  { label: t('colorSchemes.archlinux'), value: "archlinux" },
  { label: t('colorSchemes.zenburn'), value: "zenburn" },
  { label: t('colorSchemes.monokai'), value: "monokai" },
]);

const featureDescriptions: Record<string, string> = {
  random_route: t('features.randomRoute'),
  webdav: t('features.webdav'),
  download: t('features.download'),
  readme: t('features.readme'),
  hidden: t('features.hidden'),
};
</script>

<template>
  <aside class="config-panel">
    <div class="section-title"><n-icon size="16" style="margin-right: 6px;"><SettingsOutline /></n-icon> {{ t('config.basic') }}</div>

    <div class="form-row">
      <label class="form-label">{{ t('config.sharePath') }}</label>
      <div class="form-control path-row">
        <n-input :value="config.path" :placeholder="t('config.sharePathPlaceholder')" readonly size="small" style="flex: 1;" />
        <n-button type="primary" size="small" @click="emit('select-path')">
          <template #icon>
            <n-icon><FolderOpenOutline /></n-icon>
          </template>
        </n-button>
      </div>
    </div>

    <div class="form-row">
      <label class="form-label">{{ t('config.port') }}</label>
      <div class="form-control">
        <n-input-number :value="config.port" :min="1" :max="65535" size="small" style="width: 100%;" @update:value="config.port = $event ?? config.port" />
      </div>
    </div>

    <div class="form-row">
      <label class="form-label">{{ t('config.interface') }}</label>
      <div class="form-control">
        <n-select :value="config.interfaces" :options="interfaceOptions" size="small" @update:value="config.interfaces = $event" />
      </div>
    </div>

    <div class="section-title"><n-icon size="15" style="margin-right: 5px;"><LockClosedOutline /></n-icon> {{ t('config.security') }}</div>

    <div class="form-row">
      <label class="form-label">{{ t('config.username') }}</label>
      <div class="form-control">
        <n-input :value="config.auth_username" :placeholder="t('config.usernamePlaceholder')" size="small" @update:value="config.auth_username = $event" />
      </div>
    </div>

    <div class="form-row">
      <label class="form-label">{{ t('config.password') }}</label>
      <div class="form-control">
        <n-input
          :value="config.auth_password"
          type="password"
          show-password-on="click"
          :placeholder="t('config.passwordPlaceholder')"
          size="small"
          @update:value="config.auth_password = $event"
        />
      </div>
    </div>

    <div class="toggle-row">
      <TogglePill v-model="config.upload" :label="t('config.upload')" />
      <TogglePill v-if="config.upload" v-model="config.mkdir" :label="t('config.mkdir')" />
    </div>

    <div class="section-title"><n-icon size="14" style="margin-right: 4px;"><ImageOutline /></n-icon> {{ t('config.display') }}</div>

    <div class="form-row">
      <label class="form-label">{{ t('config.colorScheme') }}</label>
      <div class="form-control">
        <n-select :value="config.color_scheme" :options="colorSchemeOptions" :placeholder="t('config.colorSchemePlaceholder')" size="small" @update:value="config.color_scheme = $event" />
      </div>
    </div>

    <div class="form-row">
      <label class="form-label">{{ t('config.title') }}</label>
      <div class="form-control">
        <n-input :value="config.title" size="small" @update:value="config.title = $event" />
      </div>
    </div>

    <div class="section-title"><n-icon size="15" style="margin-right: 5px;"><FlashOutline /></n-icon> {{ t('config.advanced') }}</div>
    <div class="two-col">
      <TogglePill v-model="config.random_route" :label="t('config.randomRoute')" feature-key="random_route" @hover="emit('update:hovered-feature', $event)" />
      <TogglePill v-model="config.webdav" :label="t('config.webdav')" feature-key="webdav" @hover="emit('update:hovered-feature', $event)" />
      <TogglePill v-model="config.download" :label="t('config.download')" feature-key="download" @hover="emit('update:hovered-feature', $event)" />
      <TogglePill v-model="config.readme" :label="t('config.readme')" feature-key="readme" @hover="emit('update:hovered-feature', $event)" />
      <TogglePill v-model="config.hidden" :label="t('config.hidden')" feature-key="hidden" @hover="emit('update:hovered-feature', $event)" />
    </div>

    <div class="feature-hint">
      <span>{{ hoveredFeature ? featureDescriptions[hoveredFeature] || '' : '' }}</span>
    </div>
  </aside>
</template>

<style scoped>
.config-panel {
  width: 300px;
  min-width: 280px;
  background: var(--bg-card);
  padding: 10px 12px 36px;
  overflow-y: auto;
  border-right: 1px solid var(--border-color);
  position: relative;
  transition: all var(--transition-speed) ease;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.form-label {
  flex-shrink: 0;
  width: 90px;
  font-size: 12px;
  color: var(--text-muted);
  font-weight: 500;
  text-align: right;
}

.form-control {
  flex: 1;
  min-width: 0;
}

.path-row {
  display: flex;
  gap: 6px;
}

.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin: 4px 0 8px;
  padding-left: 98px;
}

.two-col {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 4px 0 8px;
  padding-left: 98px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--primary-color);
  margin: 12px 0 8px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-color);
}

.feature-hint {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 8px 12px;
  font-size: 11px;
  color: var(--text-muted);
  background: var(--bg-glass);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border-top: 1px solid var(--border-color);
  line-height: 1.4;
  z-index: 1;
}
</style>
