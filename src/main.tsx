import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";
import * as theme from "./lib/theme";

// First paint: use the cached saved theme so a themed user never sees a flash of the default.
// The store re-resolves flag > env > saved > default once settings load.
const cached = theme.readCached();
if (cached) theme.setActive(theme.find(cached) ?? 0);

if (import.meta.env.VITE_MOCK) {
  await import("./dev/mockTauri");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
