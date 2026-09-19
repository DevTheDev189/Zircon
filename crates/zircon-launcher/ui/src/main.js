import { createApp } from 'vue';
import App from './App.vue';
import './style.css';
import { initTheme } from './lib/theme';
import { initAlerts } from './lib/alerts';

initTheme();
initAlerts();

createApp(App).mount('#app');

