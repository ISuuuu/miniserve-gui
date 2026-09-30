import { createI18n } from 'vue-i18n';
import zhCN from './zh-CN';
import en from './en';

export type LanguageSetting = 'auto' | 'zh-CN' | 'en';

export function resolveLocale(setting?: string | null): 'zh-CN' | 'en' {
  if (setting === 'zh-CN' || setting === 'en') {
    return setting;
  }
  const sysLang = navigator.language || '';
  return sysLang.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
}

export function getSavedLanguageSetting(): LanguageSetting {
  const saved = localStorage.getItem('language');
  if (saved === 'zh-CN' || saved === 'en' || saved === 'auto') {
    return saved;
  }
  return 'auto';
}

const i18n = createI18n({
  legacy: false,
  locale: resolveLocale(getSavedLanguageSetting()),
  fallbackLocale: 'en',
  messages: {
    'zh-CN': zhCN,
    'en': en,
  },
});

export default i18n;
