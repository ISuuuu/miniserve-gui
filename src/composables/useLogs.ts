import { ref } from "vue";
import type { LogItem } from "../types";

export function useLogs(maxLogs = 200) {
  const logs = ref<LogItem[]>([]);
  let buffer: LogItem[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  let nextId = 1;

  function flushLogs() {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    if (buffer.length === 0) return;

    const merged = logs.value.concat(buffer);
    buffer = [];
    if (merged.length > maxLogs) {
      logs.value = merged.slice(-maxLogs);
    } else {
      logs.value = merged;
    }
  }

  function addLog(msg: string) {
    addLogs([msg]);
  }

  function addLogs(msgs: string[]) {
    if (msgs.length === 0) return;
    for (const msg of msgs) {
      buffer.push({
        id: nextId++,
        text: msg,
      });
    }
    if (!flushTimer) {
      flushTimer = setTimeout(flushLogs, 50);
    }
  }

  function clearLogs() {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    buffer = [];
    logs.value = [];
  }

  function cleanup() {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    buffer = [];
  }

  return { logs, addLog, addLogs, clearLogs, cleanup };
}
