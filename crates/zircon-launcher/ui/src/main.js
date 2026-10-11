import { createApp } from 'vue';
import App from './App.vue';
import './style.css';
import { initTheme } from './lib/theme';
import { initAccessibility } from './lib/accessibility';
import { initAlerts } from './lib/alerts';
import { i18nPlugin } from './lib/i18n';

initTheme();
initAccessibility();
initAlerts();

createApp(App).use(i18nPlugin).mount('#app');

