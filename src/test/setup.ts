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

// Radix Toast tracks pointer capture; jsdom elements lack hasPointerCapture.
if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = () => false;
}
if (!Element.prototype.setPointerCapture) {
  Element.prototype.setPointerCapture = () => {};
}
if (!Element.prototype.releasePointerCapture) {
  Element.prototype.releasePointerCapture = () => {};
}

// @tanstack/react-virtual reads the scroll container's offsetHeight to size its
// viewport and returns an empty range when it measures 0 (jsdom has no layout).
// Report a viewport height for the library's scroll container, the import
// preview list, and an estimate for virtualized rows so windowing behaves like
// a real browser in tests.
Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
  configurable: true,
  get(this: HTMLElement) {
    if (this.getAttribute("role") === "list" && this.getAttribute("aria-label") === "Library") {
      return 800;
    }
    if (this.getAttribute("role") === "list" && this.dataset.importPreview !== undefined) {
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
