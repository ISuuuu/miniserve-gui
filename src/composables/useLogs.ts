import { ref } from "vue";

export function useLogs(maxLogs = 200) {
  const logs = ref<string[]>([]);

  function addLog(msg: string) {
    logs.value.push(msg);
    if (logs.value.length > maxLogs) logs.value.shift();
  }

  function clearLogs() {
    logs.value = [];
  }

  return { logs, addLog, clearLogs };
}
