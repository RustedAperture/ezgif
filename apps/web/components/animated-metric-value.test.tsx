import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AnimatedMetricValue } from "./animated-metric-value";

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  cleanup();
  window.matchMedia = originalMatchMedia;
  vi.restoreAllMocks();
});

describe("AnimatedMetricValue", () => {
  it("renders the final formatted value when reduced motion is preferred", () => {
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    render(
      <AnimatedMetricValue
        value={24800}
        formatValue={(value) => `${(value / 1000).toFixed(1)} MiB`}
      />,
    );

    expect(screen.getByRole("status").textContent).toBe("24.8 MiB");
  });

  it("renders an em dash for a null value without calling the numeric formatter", () => {
    const formatValue = vi.fn((value: number) => String(value));

    render(<AnimatedMetricValue value={null} formatValue={formatValue} />);

    expect(screen.getByRole("status").textContent).toBe("—");
    expect(formatValue).not.toHaveBeenCalled();
  });
});
