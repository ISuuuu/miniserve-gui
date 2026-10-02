import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message, dialog } from "@/utils/discrete";
import { useI18n } from "vue-i18n";
import type { useLogs } from "./useLogs";

interface UpdaterConfig {
  endpoints: string[];
  proxy: string | null;
}

function getPlatform(): string {
  // @ts-ignore userAgentData is not in all TS lib versions
  const uaData = navigator.userAgentData;
  if (uaData?.platform) {
    const p = uaData.platform.toLowerCase();
    if (p.includes("win")) return "windows";
    if (p.includes("mac")) return "darwin";
    if (p.includes("linux")) return "linux";
  }
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("win")) return "windows";
  if (platform.includes("mac")) return "darwin";
  return "linux";
}

function getArch(): string {
  // @ts-ignore userAgentData is not in all TS lib versions
  const uaData = navigator.userAgentData;
  if (uaData?.platform) {
    const arch = uaData.architecture || "";
    if (arch.includes("arm") || arch.includes("aarch64")) return "aarch64";
  }
  return "x86_64";
}

export function useUpdater(
  appVersion: () => string,
  logs: ReturnType<typeof useLogs>,
) {
  const { t } = useI18n();
  const checkingUpdate = ref(false);
  const updateDownloading = ref(false);
  const updateProgress = ref(0);

  /** Translate Rust error codes to user-facing i18n messages */
  function translateUpdateError(raw: string): string {
    const errorMap: Record<string, (detail: string) => string> = {
      NON_PORTABLE_INSTALL: () => t("update.error.nonPortableInstall"),
      DOWNLOAD_FAILED: (d) => t("update.error.downloadFailed", { detail: d }),
      MANIFEST_FETCH_FAILED: (d) => t("update.error.manifestFetchFailed", { status: d }),
      MANIFEST_PARSE_FAILED: (d) => t("update.error.manifestParseFailed", { detail: d }),
      SIGNATURE_MISSING: () => t("update.error.signatureMissing"),
      SIGNATURE_INVALID: () => t("update.error.signatureInvalid"),
      PUBKEY_NOT_FOUND: (d) => t("update.error.pubkeyNotFound", { detail: d }),
      PUBKEY_PARSE_FAILED: (d) => t("update.error.pubkeyParseFailed", { detail: d }),
      SIGNATURE_PARSE_FAILED: (d) => t("update.error.signatureParseFailed", { detail: d }),
      DOWNLOAD_CHUNK_FAILED: (d) => t("update.error.downloadChunkFailed", { detail: d }),
      TEMP_FILE_FAILED: (d) => t("update.error.tempFileFailed", { detail: d }),
      TEMP_FILE_WRITE_FAILED: (d) => t("update.error.tempFileFailed", { detail: d }),
      READ_TEMP_FAILED: (d) => t("update.error.tempFileFailed", { detail: d }),
      NETWORK_ERROR: (d) => t("update.error.downloadFailed", { detail: d }),
      HTTP_ERROR: (d) => t("update.error.downloadFailed", { detail: `HTTP ${d}` }),
      INSTALLER_LAUNCH_FAILED: (d) => t("update.error.installerLaunchFailed", { detail: d }),
      PKEXEC_FAILED: (d) => t("update.error.pkexecFailed", { detail: d }),
      APPIMAGE_RELAUNCH_FAILED: (d) => t("update.error.appimageRelaunchFailed", { detail: d }),
      APPIMAGE_URL_PARSE_FAILED: () => t("update.error.appimageUrlParseFailed"),
      APPIMAGE_NAME_INVALID: () => t("update.error.appimageNameInvalid"),
      INSTALL_FAILED: (d) => t("update.error.installFailed", { detail: d }),
      REPLACE_FAILED: (d) => t("update.error.replaceFailed", { detail: d }),
      PROXY_FAILED: (d) => t("update.error.proxyFailed", { detail: d }),
      PROXY_NOT_CONFIGURED: () => t("update.error.proxyNotConfigured"),
    };
    const sep = raw.indexOf(":");
    const code = sep >= 0 ? raw.substring(0, sep) : raw;
    const detail = sep >= 0 ? raw.substring(sep + 1) : "";
    const translator = errorMap[code];
    return translator ? translator(detail) : raw;
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

  async function fetchUpdateManifest(url: string) {
    return await invoke<any>("fetch_update_manifest", { url });
  }

  async function downloadAndInstallViaBackend(
    latestVersion: string,
    updateJson: any,
    packageType: string,
  ) {
    // 便携版在 latest.json 中使用独立的 target key
    const isPortable = packageType === "portable";
    const base = `${getPlatform()}-${getArch()}`;
    const platform = isPortable ? `${base}-portable` : base;
    const platformInfo = updateJson.platforms?.[platform];
    if (!platformInfo)
      throw new Error(t("update.platformNotAvailable", { platform }));

    message.success(t("update.downloading", { version: latestVersion }));
    updateDownloading.value = true;
    updateProgress.value = 0;
    const unlisten = await listen<{
      downloaded: number;
      total: number;
    }>("update-download-progress", (event) => {
      const { downloaded, total } = event.payload;
      if (total > 0) {
        updateProgress.value = Math.min(99.9, (downloaded / total) * 100);
      }
    });
    try {
      await invoke("download_and_install_update", {
        url: platformInfo.url,
        signature: platformInfo.signature,
        version: latestVersion,
      });
      updateProgress.value = 100;
      logs.addLog(t("update.downloadFinished"));
      if (isPortable) {
        // 便携版：后端已拉起 helper 进程接管替换与重启，主进程随即退出
        return;
      }
      message.success(t("update.updateComplete"));
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } finally {
      unlisten();
      updateDownloading.value = false;
    }
  }

  async function installUpdate(update: any, proxy?: string | null) {
    let installDir = "";
    try {
      installDir = await invoke("get_install_dir");
    } catch (e) {
      console.warn("Failed to get install dir:", e);
    }
    const installerArgs = installDir
      ? ["/S", `/D=${installDir}`]
      : undefined;

    message.success(t("update.downloading", { version: update.version }));
    updateDownloading.value = true;
    updateProgress.value = 0;
    let totalSize = 0;
    let downloaded = 0;
    try {
      await update.downloadAndInstall(
        (event: any) => {
          switch (event.event) {
            case "Started":
              totalSize = event.data.contentLength || 0;
              downloaded = 0;
              logs.addLog(
                t("update.downloadStarted", {
                  size: event.data.contentLength,
                }),
              );
              break;
            case "Progress":
              downloaded += event.data.chunkLength || 0;
              if (totalSize > 0) {
                updateProgress.value = Math.min(
                  99.9,
                  (downloaded / totalSize) * 100,
                );
              }
              break;
            case "Finished":
              updateProgress.value = 100;
              logs.addLog(t("update.downloadFinished"));
              break;
          }
        },
        { installerArgs, proxy: proxy ?? undefined },
      );
    } finally {
      updateDownloading.value = false;
    }
    message.success(t("update.updateComplete"));
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  }

  async function handleUnsupportedPackage(
    version: string,
    packageType: string,
  ): Promise<boolean> {
    if (packageType !== "deb") return false;
    logs.addLog(
      t("update.debOrPortableNotSupported", { version, packageType }),
    );
    dialog.info({
      title: t("about.checkUpdate"),
      content: t("update.debOrPortableNotSupported", {
        version,
        packageType:
          packageType === "deb"
            ? t("update.packageType.deb")
            : t("update.packageType.portable"),
      }),
      positiveText: t("update.goToDownload"),
      negativeText: t("update.later"),
      onPositiveClick: () => {
        const releaseUrl = t("update.releasePage");
        openUrl(releaseUrl);
      },
    });
    return true;
  }

  async function handleVersionUpdate(
    version: string,
    updateJson: any,
    packageType: string,
  ) {
    const currentVersion = appVersion().replace(/^v/, "");
    const latestVersion = (version || updateJson.version || "").replace(
      /^v/,
      "",
    );
    if (!latestVersion || latestVersion === currentVersion) {
      message.info(t("update.alreadyLatest"));
      return;
    }

    if (await handleUnsupportedPackage(latestVersion, packageType)) return;

    logs.addLog(
      t("update.newVersion", {
        version: latestVersion,
        current: currentVersion,
      }),
    );

    await downloadAndInstallViaBackend(latestVersion, updateJson, packageType);
  }

  async function checkForUpdates() {
    if (checkingUpdate.value) return;
    checkingUpdate.value = true;
    logs.addLog(t("update.checking"));

    try {
      const updaterConfig =
        await invoke<UpdaterConfig>("get_updater_config");
      const originalUrl = updaterConfig.endpoints[0] || "";
      const packageType = await invoke<string>("get_package_type");

      // Try Tauri plugin updater first (with 5s timeout)
      let update = null;
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const checkOptions = updaterConfig.proxy
          ? { proxy: updaterConfig.proxy }
          : undefined;
        update = await Promise.race([
          check(checkOptions),
          new Promise<null>((_, reject) =>
            setTimeout(() => reject(new Error("Timeout")), 5000),
          ),
        ]);
      } catch {
        if (updaterConfig.proxy) {
          logs.addLog(
            t("update.directConnectTimeout", {
              proxy: updaterConfig.proxy,
            }),
          );
        }
      }

      if (update) {
        // Plugin updater found an update — NSIS uses native install with progress,
        // other package types fall through to manifest-based path
        if (await handleUnsupportedPackage(update.version, packageType))
          return;
        if (packageType !== "appimage" && packageType !== "portable") {
          // NSIS installer: use plugin's native download with progress tracking
          await installUpdate(update, updaterConfig.proxy);
          return;
        }
        // AppImage / portable: need manifest for platform-specific URL
        const updateJson = await fetchUpdateManifest(originalUrl);
        await handleVersionUpdate(update.version, updateJson, packageType);
      } else {
        // Fallback: fetch manifest manually and compare versions
        const updateJson = await fetchUpdateManifest(originalUrl);
        await handleVersionUpdate(
          updateJson.version || "",
          updateJson,
          packageType,
        );
      }
    } catch (e: any) {
      const rawError = e.message || e.toString() || String(e);
      const translated = translateUpdateError(rawError);
      logs.addLog(t("update.checkFailed", { error: translated }));
      message.error(t("update.checkFailed", { error: translated }));
    } finally {
      checkingUpdate.value = false;
    }
  }

  return {
    checkingUpdate,
    updateDownloading,
    updateProgress,
    checkForUpdates,
  };
}
