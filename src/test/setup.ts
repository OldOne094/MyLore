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

afterEach(() => {
  cleanup();
});
