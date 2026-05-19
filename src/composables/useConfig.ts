import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ServerConfig } from "../types";

export function useConfig() {
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

  // upload 关闭时联动关闭 mkdir
  watch(() => config.upload, (val) => {
    if (!val) config.mkdir = false;
  });

  // 500ms 防抖自动保存
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

  function cleanup() {
    if (saveTimeout) {
      clearTimeout(saveTimeout);
      saveTimeout = null;
    }
  }

  return { config, loadConfig, saveConfig, cleanup };
}
