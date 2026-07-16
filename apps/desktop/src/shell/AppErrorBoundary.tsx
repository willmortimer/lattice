import { Component, type ErrorInfo, type ReactNode } from "react";

interface AppErrorBoundaryState {
  error: Error | null;
}

export class AppErrorBoundary extends Component<
  { children: ReactNode },
  AppErrorBoundaryState
> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Lattice renderer crashed", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <main className="fatal-error">
        <p className="home-eyebrow">Renderer problem</p>
        <h1>Lattice could not draw this workspace.</h1>
        <p>
          Your workspace files were not changed. Reload the window or open the Web Inspector from
          the Developer menu for the full stack trace.
        </p>
        <pre>{this.state.error.stack ?? this.state.error.message}</pre>
        <div>
          <button className="primary-button" onClick={() => window.location.reload()}>
            Reload window
          </button>
          <button
            className="secondary-button"
            onClick={() => {
              localStorage.clear();
              window.location.reload();
            }}
          >
            Reset local UI state
          </button>
        </div>
      </main>
    );
  }
}
