import { describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDebouncedValue } from "./useDebouncedValue";

describe("useDebouncedValue", () => {
  it("lags the input value by the delay", () => {
    vi.useFakeTimers();
    try {
      const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 200), {
        initialProps: { value: "a" },
      });
      expect(result.current).toBe("a");

      rerender({ value: "ab" });
      // Not debounced yet — still serving the previous value.
      expect(result.current).toBe("a");

      act(() => {
        vi.advanceTimersByTime(200);
      });
      expect(result.current).toBe("ab");
    } finally {
      vi.useRealTimers();
    }
  });

  it("collapses rapid keystrokes into one update (MISSION-094)", () => {
    vi.useFakeTimers();
    try {
      const { result, rerender } = renderHook(({ value }) => useDebouncedValue(value, 200), {
        initialProps: { value: "" },
      });

      // Type "steins" one character at a time, well within the delay window.
      for (const char of ["s", "st", "ste", "stein", "steins"]) {
        rerender({ value: char });
        vi.advanceTimersByTime(150);
      }

      // The trailing timer finally elapses: exactly one settled value.
      act(() => {
        vi.advanceTimersByTime(200);
      });
      expect(result.current).toBe("steins");
    } finally {
      vi.useRealTimers();
    }
  });
});
