import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { QrResponse } from "../types";

export function useQr() {
  const qrCodes = ref<string[]>([]);

  async function generateQr(url: string): Promise<string> {
    try {
      const resp = await invoke<QrResponse>("generate_qr", { data: url });
      return resp.data;
    } catch (e) {
      console.error("QR generation failed:", e);
      return "";
    }
  }

  async function generateQrCodes(urls: string[]) {
    qrCodes.value = await Promise.all(urls.map((url) => generateQr(url)));
  }

  function clearQrCodes() {
    qrCodes.value = [];
  }

  return { qrCodes, generateQr, generateQrCodes, clearQrCodes };
}
