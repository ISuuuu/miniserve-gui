import { createDiscreteApi, darkTheme, lightTheme } from "naive-ui";
import { ref, watchEffect } from "vue";

export const isDarkTheme = ref(localStorage.getItem("theme") === "dark");

const configProviderPropsRef = ref({
  theme: isDarkTheme.value ? darkTheme : lightTheme,
});

watchEffect(() => {
  configProviderPropsRef.value = {
    theme: isDarkTheme.value ? darkTheme : lightTheme,
  };
});

export const { message, dialog, notification } = createDiscreteApi(
  ["message", "dialog", "notification"],
  {
    configProviderProps: configProviderPropsRef,
  },
);
