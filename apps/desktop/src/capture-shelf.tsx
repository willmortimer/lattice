import React from "react";
import ReactDOM from "react-dom/client";

import { CaptureShelfApp } from "./CaptureShelfApp";
import { markPlatform } from "./lib/platform";
import { AppErrorBoundary } from "./shell/AppErrorBoundary";
import "./styles.css";
import "./capture-shelf.css";

markPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <CaptureShelfApp />
    </AppErrorBoundary>
  </React.StrictMode>,
);
