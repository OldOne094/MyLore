import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// Radix positioning primitives inquire element size in jsdom.
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

globalThis.ResizeObserver ??= ResizeObserverMock as unknown as typeof ResizeObserver;

// @tanstack/react-virtual reads the scroll container's offsetHeight to size its
// viewport and returns an empty range when it measures 0 (jsdom has no layout).
// Report a viewport height for the library's scroll container and an estimate
// for virtualized rows so windowing behaves like a real browser in tests.
Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
  configurable: true,
  get(this: HTMLElement) {
    if (this.getAttribute("role") === "list" && this.getAttribute("aria-label") === "Library") {
      return 800;
    }
    if (this.dataset.index !== undefined) {
      return 320;
    }
    return 0;
  },
});

afterEach(() => {
  cleanup();
});
