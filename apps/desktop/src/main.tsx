import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { markPlatform } from "./lib/platform";
import { AppErrorBoundary } from "./shell/AppErrorBoundary";
import "./styles.css";

markPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <App />
    </AppErrorBoundary>
  </React.StrictMode>,
);
