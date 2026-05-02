import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import * as ElementPlusIconsVue from "@element-plus/icons-vue";
import App from "./App.vue";
import i18n from "./i18n";

// 禁用右键菜单（客户端不需要浏览器右键菜单）
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(App);
app.use(ElementPlus);
app.use(i18n);
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component);
}
app.mount("#app");
