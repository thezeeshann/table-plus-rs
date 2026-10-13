import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";

if (import.meta.env.VITE_MOCK) {
  await import("./dev/mockTauri");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
