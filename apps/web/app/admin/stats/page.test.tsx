import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/components/app-shell", () => ({
  AppShell: ({ children }: { children: React.ReactNode }) => <div data-testid="app-shell">{children}</div>,
}));

vi.mock("@/components/admin-area-layout", () => ({
  AdminAreaLayout: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="admin-area-layout">{children}</div>
  ),
}));

vi.mock("@/components/require-admin-stats", () => ({
  RequireAdminStats: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="require-admin-stats">{children}</div>
  ),
}));

vi.mock("@/components/admin-stats-dashboard", () => ({
  AdminStatsDashboard: () => <div data-testid="admin-stats-dashboard">dashboard</div>,
}));

import AdminStatsPage from "@/app/admin/stats/page";

describe("AdminStatsPage", () => {
  it("renders the page title, description, and dashboard inside the stats guard without re-wrapping the shared layout", () => {
    render(<AdminStatsPage />);

    expect(screen.getByTestId("require-admin-stats")).toBeTruthy();
    expect(screen.queryByTestId("app-shell")).toBeNull();
    expect(screen.queryByTestId("admin-area-layout")).toBeNull();
    expect(screen.getByRole("heading", { name: "Admin stats" })).toBeTruthy();
    expect(
      screen.getByText("Review growth, activity, and storage trends for the app."),
    ).toBeTruthy();
    expect(screen.getByTestId("admin-stats-dashboard")).toBeTruthy();
  });
});
