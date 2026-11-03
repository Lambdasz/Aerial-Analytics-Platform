import { Component, type ReactNode } from "react";
import { Callout, Intent } from "@blueprintjs/core";
import { PluginManager } from "../../component_plugin_manager/PluginManager";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class PluginManagerErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = {
    hasError: false,
    error: null,
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  override render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: 24 }}>
          <Callout intent={Intent.DANGER} title="Module 02 (Plugin Manager) Encountered an Error">
            <p>An unexpected error occurred while rendering the Plugin Manager interface:</p>
            <pre style={{ margin: "8px 0" }}>
              {this.state.error?.message ?? "Unknown rendering error."}
            </pre>
          </Callout>
        </div>
      );
    }

    return this.props.children;
  }
}

export function Module02Page() {
  return (
    <PluginManagerErrorBoundary>
      <PluginManager />
    </PluginManagerErrorBoundary>
  );
}
