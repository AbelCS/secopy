import { mount } from "svelte";
import App from "./App.svelte";
import Panel from "./menubar/Panel.svelte";
import "./app.css";
import { tauriApi } from "./lib/api";
import { setLocale } from "./lib/i18n";

// The same page is the main window and, with `?panel`, the menu bar panel (#80).
const panel = new URLSearchParams(location.search).has("panel");
if (panel) {
  // The panel's window is see-through: only its rounded card shows. Set here, not in a
  // stylesheet, so no later rule paints the page's background again.
  for (const el of [document.documentElement, document.body]) el.style.background = "transparent";
}
// Secopy's language (#181) before anything is drawn; Settings changing it draws again.
try {
  setLocale(await tauriApi.appLanguage());
} catch {
  // The Mac's language, as chosen at load: still a full catalog.
}
void tauriApi.onLanguageChanged(() => location.reload());

const app = mount(panel ? Panel : App, { target: document.getElementById("app")! });

export default app;
