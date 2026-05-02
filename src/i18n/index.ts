import { createI18n } from 'vue-i18n';
import zhCN from './zh-CN';
import en from './en';

const i18n = createI18n({
  legacy: false,
  locale: navigator.language.startsWith('zh') ? 'zh-CN' : 'en',
  fallbackLocale: 'en',
  messages: {
    'zh-CN': zhCN,
    'en': en,
  },
});

export default i18n;
