import React from "react";
import ReactDOM from "react-dom/client";

import { QuickNoteApp } from "./QuickNoteApp";
import { markPlatform } from "./lib/platform";
import { AppErrorBoundary } from "./shell/AppErrorBoundary";
import "./styles.css";
import "./quick-note.css";

markPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <QuickNoteApp />
    </AppErrorBoundary>
  </React.StrictMode>,
);
