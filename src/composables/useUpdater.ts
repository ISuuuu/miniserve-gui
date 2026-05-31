import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage, ElMessageBox } from "element-plus";
import { useI18n } from "vue-i18n";
import type { useLogs } from "./useLogs";

interface UpdaterConfig {
  endpoints: string[];
  proxy: string | null;
}

function getPlatform(): string {
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

  async function openUrl(url: string) {
    try {
      const { openUrl: tauriOpenUrl } = await import("@tauri-apps/plugin-opener");
      await tauriOpenUrl(url);
    } catch (e) {
      console.error("Failed to open URL:", e);
      ElMessage.error(t("messages.openUrlFailed", { error: e }));
    }
  }

  async function fetchUpdateManifest(url: string) {
    return await invoke<any>("fetch_update_manifest", { url });
  }

  async function installAppImageUpdate(
    version: string,
    originalUrl: string,
  ) {
    const updateJson = await fetchUpdateManifest(originalUrl);
    const latestVersion = (
      version ||
      updateJson.version ||
      ""
    ).replace(/^v/, "");
    const platform = `${getPlatform()}-${getArch()}`;
    const platformInfo = updateJson.platforms?.[platform];
    if (!platformInfo)
      throw new Error(t("update.platformNotAvailable", { platform }));

    ElMessage.success(t("update.downloading", { version: latestVersion }));
    await invoke("download_and_install_update", {
      url: platformInfo.url,
      signature: platformInfo.signature,
      version: latestVersion,
    });
  }

  async function installUpdate(update: any) {
    let installDir = "";
    try {
      installDir = await invoke("get_install_dir");
    } catch (e) {
      console.warn("无法获取安装目录:", e);
    }
    const installerArgs = installDir
      ? ["/S", `/D=${installDir}`]
      : undefined;

    ElMessage.success(t("update.downloading", { version: update.version }));
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
              logs.addLog(
                t("update.downloadProgress", {
                  size: event.data.chunkLength,
                }),
              );
              break;
            case "Finished":
              updateProgress.value = 100;
              logs.addLog(t("update.downloadFinished"));
              break;
          }
        },
        { installerArgs },
      );
    } finally {
      updateDownloading.value = false;
    }
    ElMessage.success(t("update.updateComplete"));
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  }

  async function checkForUpdates() {
    if (checkingUpdate.value) return;
    checkingUpdate.value = true;
    logs.addLog(t("update.checking"));

    try {
      const updaterConfig =
        await invoke<UpdaterConfig>("get_updater_config");
      const originalUrl = updaterConfig.endpoints[0] || "";
      const proxyPrefix = updaterConfig.proxy || "";

      const { check } = await import("@tauri-apps/plugin-updater");

      let update = null;
      try {
        update = await Promise.race([
          check(),
          new Promise<null>((_, reject) =>
            setTimeout(() => reject(new Error("Timeout")), 5000),
          ),
        ]);
      } catch (e) {
        if (proxyPrefix) {
          logs.addLog(
            t("update.directConnectTimeout", { proxy: proxyPrefix }),
          );
        }
        const updateJson = await invoke<any>("fetch_update_manifest", { url: originalUrl });

        const currentVersion = appVersion().replace(/^v/, "");
        const latestVersion = (updateJson.version || "").replace(
          /^v/,
          "",
        );

        if (latestVersion && latestVersion !== currentVersion) {
          const packageType = await invoke<string>("get_package_type");
          if (packageType === "deb" || packageType === "portable") {
            logs.addLog(
              t("update.debOrPortableNotSupported", {
                version: latestVersion,
                packageType,
              }),
            );
            ElMessageBox.confirm(
              t("update.debOrPortableNotSupported", {
                version: latestVersion,
                packageType:
                  packageType === "deb"
                    ? "DEB安装版"
                    : "Windows便携版",
              }),
              "发现更新",
              {
                confirmButtonText: t("update.goToDownload"),
                cancelButtonText: t("update.later"),
                type: "info",
              },
            )
              .then(() => {
                const releaseUrl = t("update.releasePage");
                openUrl(releaseUrl);
              })
              .catch(() => {});
            return;
          }

          logs.addLog(
            t("update.newVersion", {
              version: latestVersion,
              current: currentVersion,
            }),
          );
          const platform = `${getPlatform()}-${getArch()}`;
          const platformInfo = updateJson.platforms?.[platform];
          if (!platformInfo)
            throw new Error(
              t("update.platformNotAvailable", { platform }),
            );

          ElMessage.success(
            t("update.downloading", { version: latestVersion }),
          );
          await invoke("download_and_install_update", {
            url: platformInfo.url,
            signature: platformInfo.signature,
            version: latestVersion,
          });
          return;
        }
        update = null;
      }

      if (update) {
        const packageType = await invoke<string>("get_package_type");
        if (packageType === "deb" || packageType === "portable") {
          logs.addLog(
            t("update.debOrPortableNotSupported", {
              version: update.version,
              packageType,
            }),
          );
          ElMessageBox.confirm(
            t("update.debOrPortableNotSupported", {
              version: update.version,
              packageType:
                packageType === "deb"
                  ? "DEB安装版"
                  : "Windows便携版",
            }),
            "发现更新",
            {
              confirmButtonText: t("update.goToDownload"),
              cancelButtonText: t("update.later"),
              type: "info",
            },
          )
            .then(() => {
              const releaseUrl = t("update.releasePage");
              openUrl(releaseUrl);
            })
            .catch(() => {});
          return;
        }
        if (packageType === "appimage") {
          await installAppImageUpdate(
            update.version,
            originalUrl,
          );
          return;
        }
        await installUpdate(update);
      } else {
        ElMessage.info(t("update.alreadyLatest"));
      }
    } catch (e: any) {
      logs.addLog(t("update.checkFailed", { error: e }));
      ElMessage.error(
        t("update.checkFailed", { error: e.message || e }),
      );
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
