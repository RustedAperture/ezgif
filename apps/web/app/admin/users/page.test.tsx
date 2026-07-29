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

vi.mock("@/components/require-manage-permissions", () => ({
  RequireManagePermissions: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="require-manage-permissions">{children}</div>
  ),
}));

vi.mock("@/components/admin-users-table", () => ({
  AdminUsersTable: () => <div data-testid="admin-users-table">users</div>,
}));

import AdminUsersPage from "@/app/admin/users/page";

describe("AdminUsersPage", () => {
  it("renders the page title, description, and table inside the manage-permissions guard without re-wrapping the shared layout", () => {
    render(<AdminUsersPage />);

    expect(screen.getByTestId("require-manage-permissions")).toBeTruthy();
    expect(screen.queryByTestId("app-shell")).toBeNull();
    expect(screen.queryByTestId("admin-area-layout")).toBeNull();
    expect(screen.getByRole("heading", { name: "Admin users" })).toBeTruthy();
    expect(screen.getByText("Manage roles and permissions for existing users.")).toBeTruthy();
    expect(screen.getByTestId("admin-users-table")).toBeTruthy();
  });
});
