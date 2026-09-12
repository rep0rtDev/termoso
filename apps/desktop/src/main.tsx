import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter";
import "@xterm/xterm/css/xterm.css";
import "./terminal/fonts";
import "./app.css";
import { App } from "./App";

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");

// A drop nobody handled must never navigate the webview to the dropped file.
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, (e) => e.preventDefault());
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
