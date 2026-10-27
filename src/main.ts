import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { currentOs, hasNativeGlass } from "$lib/platform";

const os = currentOs();
document.documentElement.dataset.os = os;
document.documentElement.dataset.glass = hasNativeGlass(os) ? "native" : "drawn";

export default mount(App, { target: document.getElementById("app")! });
