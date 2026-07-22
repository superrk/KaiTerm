import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "@fortawesome/fontawesome-free/css/all.min.css";
import "xterm/css/xterm.css";

const pinia = createPinia();
const app = createApp(App);
app.use(pinia);
app.mount("#app");
