import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShell } from "@/components/app-shell";

const mocks = vi.hoisted(() => ({
  useUser: vi.fn(),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("@/components/account-modal", () => ({
  AccountModal: () => <div>Account</div>,
}));

vi.mock("@/components/theme-toggle", () => ({
  ThemeToggle: () => <div>Theme</div>,
}));

function adminUser({
  isRootAdmin = false,
  managePermissions = false,
  viewAdminStats = false,
}: {
  isRootAdmin?: boolean;
  managePermissions?: boolean;
  viewAdminStats?: boolean;
}) {
  return {
    user: {
      id: "admin-1",
      username: "admin",
      display_name: null,
      avatar_url: null,
      role: "admin" as const,
      is_root_admin: isRootAdmin,
      permissions: {
        upload_local_images: false,
        manage_permissions: managePermissions,
        view_admin_stats: viewAdminStats,
      },
    },
  };
}

describe("AppShell", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.useUser.mockReset();
  });

  it("hides the admin link for an admin without admin-area access", () => {
    mocks.useUser.mockReturnValue(adminUser({}));

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.queryByRole("link", { name: "Admin" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();
  });

  it("uses /admin/users for the top-level admin link when the user can manage permissions", () => {
    mocks.useUser.mockReturnValue(adminUser({ managePermissions: true }));

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Admin" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();
  });

  it("uses /admin/stats for the top-level admin link when stats is the only admin-area access", () => {
    mocks.useUser.mockReturnValue(adminUser({ viewAdminStats: true }));

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Admin" }).getAttribute("href")).toBe("/admin/stats");
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();
  });

  it("prefers /admin/users for a root admin", () => {
    mocks.useUser.mockReturnValue(adminUser({ isRootAdmin: true }));

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Admin" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();
  });
});
