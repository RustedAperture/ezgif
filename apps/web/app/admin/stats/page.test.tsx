import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
  it("renders the page title, description, and dashboard inside the stats guard", () => {
    render(<AdminStatsPage />);

    expect(screen.getByTestId("require-admin-stats")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Admin stats" })).toBeTruthy();
    expect(
      screen.getByText("Review growth, activity, and storage trends for the app."),
    ).toBeTruthy();
    expect(screen.getByTestId("admin-stats-dashboard")).toBeTruthy();
  });
});
