import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@/utils/discrete";
import { useI18n } from "vue-i18n";
import type { EngineStatus } from "../types";

export function useEngine() {
  const { t } = useI18n();
  const engineStatus = ref<EngineStatus | null>(null);
  const downloading = ref(false);
  const progress = ref(0);

  async function checkEngine() {
    try {
      engineStatus.value = await invoke<EngineStatus>("get_engine_status");
      if (engineStatus.value && !engineStatus.value.exists) {
        message.info(t("messages.engineNotInstalledInfo"));
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function downloadEngine() {
    downloading.value = true;
    progress.value = 0;
    message.info(t("messages.startDownloadEngine"));
    try {
      const result = await invoke<string>("download_engine");
      downloading.value = false;
      message.success(t("messages.downloadEngineSuccess", { result }));
      await checkEngine();
    } catch (e) {
      downloading.value = false;
      message.error(t("messages.downloadEngineFailed", { error: e }));
    }
  }

  return { engineStatus, downloading, progress, checkEngine, downloadEngine };
}
