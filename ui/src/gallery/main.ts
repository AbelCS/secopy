// The design-system gallery (docs/design/design-system.md): every component, and every screen
// with fake data. Dev only: `npm run dev`, then open /gallery.html. Not part of the app build.
import { mount } from "svelte";
import "../app.css";
import Gallery from "./Gallery.svelte";

mount(Gallery, { target: document.getElementById("app")! });
