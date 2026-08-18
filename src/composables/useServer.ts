import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@/utils/discrete";
import { useI18n } from "vue-i18n";
import type { ServerConfig, ServerStatus } from "../types";
import { useQr } from "./useQr";
import type { useLogs } from "./useLogs";

export function useServer(
  config: ServerConfig,
  engineExists: () => boolean,
  logs: ReturnType<typeof useLogs>,
) {
  const { t } = useI18n();
  const serverStatus = ref<ServerStatus | null>(null);
  const serverUrls = ref<string[]>([]);
  const loading = ref(false);
  const { qrCodes, generateQrCodes, clearQrCodes } = useQr();

  async function startServer() {
    if (!config.path) {
      message.warning(t("messages.selectFolderFirst"));
      return;
    }
    if (!engineExists()) {
      message.warning(t("messages.downloadEngineFirst"));
      return;
    }
    loading.value = true;
    logs.addLog(t("messages.startingService"));
    try {
      const status = await invoke<ServerStatus>("start_server", {
        config: { ...config },
      });
      serverStatus.value = status;
      logs.addLog(
        t("messages.startComplete", { status: JSON.stringify(status) }),
      );

      const urlsToShow =
        status.urls && status.urls.length > 0
          ? status.urls
          : status.url
            ? [status.url]
            : [];
      if (urlsToShow.length > 0) {
        logs.addLog(t("messages.serviceStarted", { urls: urlsToShow.join(", ") }));
        message.success(t("messages.serviceStarted", { urls: "" }));
        serverUrls.value = urlsToShow;
        // Background async QR generation to avoid delaying start completion
        generateQrCodes(urlsToShow).catch((e) => {
          console.error("Failed to generate QR codes:", e);
        });
      }
    } catch (e) {
      logs.addLog(t("messages.startFailed", { error: e }));
      message.error(t("messages.startFailed", { error: e }));
    } finally {
      loading.value = false;
      logs.addLog(t("messages.loadingReset"));
    }
  }

  async function stopServer() {
    loading.value = true;
    logs.addLog(t("messages.stoppingService"));
    try {
      await invoke("stop_server");
      serverStatus.value = {
        running: false,
        pid: null,
        url: null,
        urls: [],
        port: null,
      };
      clearQrCodes();
      serverUrls.value = [];
      logs.addLog(t("messages.serviceStopped"));
      message.info(t("messages.serviceStopped"));
    } catch (e) {
      message.error(t("messages.stopFailed", { error: e }));
    } finally {
      loading.value = false;
    }
  }

  return {
    serverStatus,
    serverUrls,
    loading,
    qrCodes,
    startServer,
    stopServer,
  };
}
