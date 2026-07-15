import { createApp } from "vue";
import App from "./App.vue";
import i18n from "./i18n";

// 禁用右键菜单（客户端不需要浏览器右键菜单）
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(App);
app.use(i18n);
app.mount("#app");
