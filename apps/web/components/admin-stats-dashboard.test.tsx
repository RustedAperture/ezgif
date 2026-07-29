import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AdminStatsDashboard } from "@/components/admin-stats-dashboard";
import type { AdminStatsResponse, AdminStatsSnapshot } from "@/lib/types";

const mocks = vi.hoisted(() => ({
  apiGet: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  apiGet: mocks.apiGet,
}));

vi.mock("@/components/ui/select", async () => {
  const React = await import("react");

  type SelectContextValue = {
    value: string;
    onValueChange?: (value: string) => void;
  };

  const SelectContext = React.createContext<SelectContextValue | null>(null);

  return {
    Select: ({
      value,
      onValueChange,
      children,
    }: {
      value: string;
      onValueChange?: (value: string) => void;
      children: React.ReactNode;
    }) => (
      <SelectContext.Provider value={{ value, onValueChange }}>
        <div>{children}</div>
      </SelectContext.Provider>
    ),
    SelectTrigger: ({ children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
      <button role="combobox" type="button" {...props}>
        {children}
      </button>
    ),
    SelectValue: ({ placeholder }: { placeholder?: string }) => {
      const context = React.useContext(SelectContext)!;
      return <span>{context.value || placeholder}</span>;
    },
    SelectContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => {
      const context = React.useContext(SelectContext)!;
      return (
        <button role="option" type="button" onClick={() => context.onValueChange?.(value)}>
          {children}
        </button>
      );
    },
  };
});

vi.mock("@/components/ui/chart", () => ({
  ChartContainer: ({
    children,
    ...props
  }: {
    children: React.ReactNode;
  }) => <div data-testid="chart-container" {...props}>{children}</div>,
  ChartTooltip: ({ children }: { children?: React.ReactNode }) => <>{children ?? null}</>,
  ChartTooltipContent: () => null,
}));

vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  CartesianGrid: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  LineChart: ({ data, children }: { data: unknown; children: React.ReactNode }) => (
    <div data-testid="line-chart">
      <pre>{JSON.stringify(data)}</pre>
      {children}
    </div>
  ),
  Line: () => null,
  BarChart: ({ data, children }: { data: unknown; children: React.ReactNode }) => (
    <div data-testid="bar-chart">
      <pre>{JSON.stringify(data)}</pre>
      {children}
    </div>
  ),
  Bar: () => null,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

function isoDate(date: Date) {
  return date.toISOString().slice(0, 10);
}

function buildHistory(days: number) {
  const end = new Date("2026-07-28T00:00:00.000Z");
  const history: AdminStatsSnapshot[] = [];
  let sendCount = 1_000;

  for (let index = 0; index < days; index += 1) {
    const date = new Date(end);
    date.setUTCDate(end.getUTCDate() - (days - 1 - index));
    const dailySends = (index % 4) + 1;
    sendCount += 10;

    history.push({
      snapshot_date: isoDate(date),
      user_count: 100 + index,
      bucket_count: 50 + index,
      image_link_count: 500 + index * 2,
      unique_file_count: 900 + index,
      send_count: sendCount,
      daily_send_count: dailySends,
      b2_object_count: 1200 + index,
      b2_bytes: 1024 * (index + 1),
    });
  }

  return history;
}

function buildResponse(overrides?: Partial<AdminStatsResponse>): AdminStatsResponse {
  const history = overrides?.history ?? buildHistory(120);
  const fallbackCurrent = buildHistory(120)[119];
  return {
    current: overrides?.current ?? history[history.length - 1] ?? fallbackCurrent,
    history,
    storage: overrides?.storage ?? {
      configured: true,
      available: true,
      first_complete_history_date: history[0]?.snapshot_date ?? null,
    },
  };
}

function readChart(testId: "line-chart" | "bar-chart") {
  return JSON.parse(screen.getByTestId(testId).querySelector("pre")!.textContent ?? "[]") as Array<{
    date: string;
    value: number | null;
  }>;
}

describe("AdminStatsDashboard", () => {
  beforeEach(() => {
    mocks.apiGet.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("loads the aggregate cards and defaults the chart to the last 30 days of users", async () => {
    const response = buildResponse();
    mocks.apiGet.mockResolvedValue(response);

    render(<AdminStatsDashboard />);

    await waitFor(() => expect(mocks.apiGet).toHaveBeenCalledWith("/api/admin/stats"));

    expect(screen.getAllByText("Users").length).toBeGreaterThan(0);
    expect(screen.getByText("219")).toBeTruthy();
    expect(screen.getAllByText("Buckets").length).toBeGreaterThan(0);
    expect(screen.getByText("169")).toBeTruthy();
    expect(screen.getAllByText("B2 storage").length).toBeGreaterThan(0);
    expect(screen.getByText("120.0 KiB")).toBeTruthy();

    const chartData = readChart("line-chart");
    expect(chartData).toHaveLength(30);
    expect(chartData[0]).toEqual({ date: "2026-06-29", value: 190 });
    expect(chartData[29]).toEqual({ date: "2026-07-28", value: 219 });
  });

  it("charts explicit daily sends and supports 7, 30, 90, and all ranges", async () => {
    const response = buildResponse();
    mocks.apiGet.mockResolvedValue(response);

    render(<AdminStatsDashboard />);
    await screen.findByText("219");

    fireEvent.click(screen.getByRole("combobox", { name: "Metric" }));
    fireEvent.click(screen.getByRole("option", { name: "Sends" }));

    let chartData = readChart("bar-chart");
    expect(chartData).toHaveLength(30);
    expect(chartData[0]).toEqual({ date: "2026-06-29", value: 3 });
    expect(chartData[29]).toEqual({ date: "2026-07-28", value: 4 });

    fireEvent.click(screen.getByRole("combobox", { name: "Range" }));
    fireEvent.click(screen.getByRole("option", { name: "7 days" }));
    chartData = readChart("bar-chart");
    expect(chartData).toHaveLength(7);
    expect(chartData[0]).toEqual({ date: "2026-07-22", value: 2 });

    fireEvent.click(screen.getByRole("combobox", { name: "Range" }));
    fireEvent.click(screen.getByRole("option", { name: "90 days" }));
    chartData = readChart("bar-chart");
    expect(chartData).toHaveLength(90);
    expect(chartData[0]).toEqual({ date: "2026-04-30", value: 3 });

    fireEvent.click(screen.getByRole("combobox", { name: "Range" }));
    fireEvent.click(screen.getByRole("option", { name: "All time" }));
    chartData = readChart("bar-chart");
    expect(chartData).toHaveLength(120);
    expect(chartData[0]).toEqual({ date: "2026-03-31", value: 1 });
  });

  it("shows loading skeletons while the stats request is in flight", () => {
    const request = deferred<AdminStatsResponse>();
    mocks.apiGet.mockReturnValue(request.promise);

    const { container } = render(<AdminStatsDashboard />);

    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
    expect(screen.queryByTestId("line-chart")).toBeNull();
    expect(screen.queryByTestId("bar-chart")).toBeNull();
  });

  it("shows a request error when the stats request is rejected", async () => {
    mocks.apiGet.mockRejectedValue(new Error("request failed"));

    render(<AdminStatsDashboard />);

    await screen.findByText("Admin stats unavailable");
    expect(screen.getByRole("alert").textContent).toContain(
      "Could not load admin stats. Try again.",
    );
  });

  it("shows an empty-history state when the API has no snapshots to chart", async () => {
    mocks.apiGet.mockResolvedValue(buildResponse({ history: [] }));

    render(<AdminStatsDashboard />);

    const emptyHistoryTitle = await screen.findByText("No historical snapshots yet.");
    expect(screen.queryByTestId("line-chart")).toBeNull();
    expect(emptyHistoryTitle.closest('[role="alert"]')?.textContent).toContain(
      "No historical snapshots yet.",
    );
  });

  it("keeps null storage history values blank and explains when storage metrics are unavailable", async () => {
    const response = buildResponse({
      current: {
        ...buildResponse().current,
        unique_file_count: null,
        b2_object_count: null,
        b2_bytes: null,
      },
      storage: {
        configured: true,
        available: false,
        first_complete_history_date: "2026-07-10",
      },
    });
    mocks.apiGet.mockResolvedValue(response);

    render(<AdminStatsDashboard />);

    await screen.findByText("Storage metrics are not available for the latest snapshot yet.");
    expect(screen.getAllByText("Unique files").length).toBeGreaterThan(0);
    expect(screen.getAllByText("B2 objects").length).toBeGreaterThan(0);
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(3);
    expect(screen.getAllByRole("alert")).toHaveLength(2);
    expect(screen.getByText("Complete storage history begins on 2026-07-10.")).toBeTruthy();
  });

  it("shows the healthy storage history start when the latest snapshot is available", async () => {
    mocks.apiGet.mockResolvedValue(buildResponse({
      storage: {
        configured: true,
        available: true,
        first_complete_history_date: "2026-07-10",
      },
    }));

    render(<AdminStatsDashboard />);

    await screen.findByText("Complete storage history begins on 2026-07-10.");
    expect(screen.queryByText("Storage metrics are not available for the latest snapshot yet.")).toBeNull();
  });
});
