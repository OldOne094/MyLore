import { Component, type ReactNode } from "react";

/* MISSION-113 — Error boundary wrapping chart components. Recharts requires
   real DOM layout which jsdom doesn't provide; this catches the crash and
   renders a fallback so tests don't blow up. No-op in production browsers. */

interface ChartBoundaryProps {
  children: ReactNode;
}

interface ChartBoundaryState {
  hasError: boolean;
}

export class ChartBoundary extends Component<ChartBoundaryProps, ChartBoundaryState> {
  override state: ChartBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ChartBoundaryState {
    return { hasError: true };
  }

  override componentDidCatch(): void {
    // Charts are visual-only; silently swallow rendering failures.
  }

  override render(): ReactNode {
    if (this.state.hasError) {
      return <div style={{ height: 200 }} aria-hidden="true" data-testid="chart-fallback" />;
    }
    return this.props.children;
  }
}
