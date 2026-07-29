import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const motionMocks = vi.hoisted(() => ({
  animate: vi.fn(),
  useReducedMotion: vi.fn(),
}));

vi.mock("framer-motion", () => ({
  animate: motionMocks.animate,
  motion: { span: "span" },
  useReducedMotion: motionMocks.useReducedMotion,
}));

import { AnimatedMetricValue } from "./animated-metric-value";

beforeEach(() => {
  motionMocks.animate.mockReset();
  motionMocks.useReducedMotion.mockReturnValue(false);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("AnimatedMetricValue", () => {
  it("renders the final formatted value when reduced motion is preferred", () => {
    motionMocks.useReducedMotion.mockReturnValue(true);

    const { container } = render(
      <AnimatedMetricValue
        value={24800}
        formatValue={(value) => `${(value / 1000).toFixed(1)} MiB`}
      />,
    );

    const status = screen.getByRole("status");
    const visualValue = container.querySelector('span[aria-hidden="true"]');

    expect(status.textContent).toBe("24.8 MiB");
    expect(status.getAttribute("aria-live")).toBe("polite");
    expect(status.getAttribute("aria-atomic")).toBe("true");
    expect(visualValue?.getAttribute("aria-hidden")).toBe("true");
    expect(motionMocks.animate).not.toHaveBeenCalled();
  });

  it("animates the hidden visual value while exposing only the final target", () => {
    let onUpdate: ((latest: number) => void) | undefined;
    const stop = vi.fn();
    motionMocks.animate.mockImplementation(
      (
        from: number,
        to: number,
        options: { onUpdate: (latest: number) => void },
      ) => {
        onUpdate = options.onUpdate;
        return { stop };
      },
    );

    const { container } = render(
      <AnimatedMetricValue value={24800} formatValue={(value) => String(value)} />,
    );

    const status = screen.getByRole("status");
    const visualValue = container.querySelector('span[aria-hidden="true"]');

    expect(motionMocks.animate).toHaveBeenCalledWith(
      0,
      24800,
      expect.objectContaining({ duration: 2 }),
    );
    expect(visualValue?.textContent).toBe("0");
    expect(status.textContent).toBe("24800");

    act(() => onUpdate?.(1234));

    expect(visualValue?.textContent).toBe("1234");
    expect(status.textContent).toBe("24800");
  });

  it("renders an em dash for a null value without calling the numeric formatter", () => {
    const formatValue = vi.fn((value: number) => String(value));

    const { container } = render(
      <AnimatedMetricValue value={null} formatValue={formatValue} />,
    );

    expect(screen.getByRole("status").textContent).toBe("—");
    expect(container.querySelector('span[aria-hidden="true"]')?.textContent).toBe("—");
    expect(formatValue).not.toHaveBeenCalled();
    expect(motionMocks.animate).not.toHaveBeenCalled();
  });
});
