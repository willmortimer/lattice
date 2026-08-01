import React from "react";
import ReactDOM from "react-dom/client";

import { AgentDetachedApp } from "./AgentDetachedApp";
import { markPlatform } from "./lib/platform";
import { AppErrorBoundary } from "./shell/AppErrorBoundary";
import "./styles.css";

markPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <AgentDetachedApp />
    </AppErrorBoundary>
  </React.StrictMode>,
);
