import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";

import { QuickNoteApp } from "./QuickNoteApp";
import { markPlatform } from "./lib/platform";
import { queryClient } from "./query/queryClient";
import { AppErrorBoundary } from "./shell/AppErrorBoundary";
import "./styles.css";
import "./quick-note.css";

markPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <QuickNoteApp />
      </QueryClientProvider>
    </AppErrorBoundary>
  </React.StrictMode>,
);
