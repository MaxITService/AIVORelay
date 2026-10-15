import React from "react";
import { logRenderCrash } from "@/lib/frontendErrorLog";

type SilentCrashBoundaryProps = {
  /** What crashed, for the log line. */
  where: string;
  /** Runs after logging, e.g. to close a window that would otherwise sit there blank. */
  onCrash?: () => void;
  children: React.ReactNode;
};

/**
 * Logs a render crash and renders nothing, for overlays and optional extras
 * where a crash card would not fit. Keeps overlay bundles free of the main
 * window's crash screen dependencies.
 */
export class SilentCrashBoundary extends React.Component<
  SilentCrashBoundaryProps,
  { hasError: boolean }
> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo) {
    logRenderCrash(this.props.where, error, info.componentStack);
    this.props.onCrash?.();
  }

  render() {
    return this.state.hasError ? null : this.props.children;
  }
}
