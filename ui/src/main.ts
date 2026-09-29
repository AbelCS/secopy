import { mount } from "svelte";
import App from "./App.svelte";
import Panel from "./menubar/Panel.svelte";
import "./app.css";

// The same page is the main window and, with `?panel`, the menu bar panel (#80).
const panel = new URLSearchParams(location.search).has("panel");
const app = mount(panel ? Panel : App, { target: document.getElementById("app")! });

export default app;
