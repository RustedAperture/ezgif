import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AdminAreaLayout } from "@/components/admin-area-layout";

const mocks = vi.hoisted(() => ({
  useUser: vi.fn(),
  usePathname: vi.fn(),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("next/navigation", () => ({
  usePathname: mocks.usePathname,
}));

vi.mock("@/components/ui/sidebar", () => {
  const React = require("react");

  return {
    SidebarProvider: ({ children }: { children: React.ReactNode }) => <div data-testid="sidebar-provider">{children}</div>,
    Sidebar: ({ children }: { children: React.ReactNode }) => <aside data-testid="sidebar">{children}</aside>,
    SidebarContent: ({ children }: { children: React.ReactNode }) => <div data-testid="sidebar-content">{children}</div>,
    SidebarGroup: ({ children }: { children: React.ReactNode }) => <section>{children}</section>,
    SidebarGroupLabel: ({ children }: { children: React.ReactNode }) => <h2>{children}</h2>,
    SidebarMenu: ({ children }: { children: React.ReactNode }) => <ul>{children}</ul>,
    SidebarMenuItem: ({ children }: { children: React.ReactNode }) => <li>{children}</li>,
    SidebarMenuButton: ({
      children,
      isActive,
      render,
    }: {
      children: React.ReactNode;
      isActive?: boolean;
      render?: React.ReactElement;
    }) =>
      render
        ? React.cloneElement(render, { "data-active": isActive ? "true" : "false", children })
        : <button data-active={isActive ? "true" : "false"}>{children}</button>,
    SidebarInset: ({ children }: { children: React.ReactNode }) => <div data-testid="sidebar-inset">{children}</div>,
    SidebarTrigger: () => <button type="button">Toggle Sidebar</button>,
  };
});

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

describe("AdminAreaLayout", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.usePathname.mockReset();
    mocks.useUser.mockReset();
    mocks.usePathname.mockReturnValue("/admin/users");
  });

  it("shows users only with manage permissions and stats only with stats access", () => {
    mocks.useUser.mockReturnValue(adminUser({ managePermissions: true }));

    const { rerender } = render(
      <AdminAreaLayout>
        <p>Child</p>
      </AdminAreaLayout>,
    );

    expect(screen.getByRole("link", { name: "Users" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();

    mocks.useUser.mockReturnValue(adminUser({ viewAdminStats: true }));
    mocks.usePathname.mockReturnValue("/admin/stats");

    rerender(
      <AdminAreaLayout>
        <p>Child</p>
      </AdminAreaLayout>,
    );

    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("href")).toBe("/admin/stats");
    expect(screen.queryByRole("link", { name: "Users" })).toBeNull();
  });

  it("marks the current admin section active", () => {
    mocks.useUser.mockReturnValue(adminUser({ viewAdminStats: true }));
    mocks.usePathname.mockReturnValue("/admin/stats");

    render(
      <AdminAreaLayout>
        <p>Child</p>
      </AdminAreaLayout>,
    );

    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("data-active")).toBe("true");
  });

  it("shows both entries for a root admin and keeps the content inset", () => {
    mocks.useUser.mockReturnValue(adminUser({ isRootAdmin: true }));

    render(
      <AdminAreaLayout>
        <p>Child</p>
      </AdminAreaLayout>,
    );

    expect(screen.getByRole("link", { name: "Users" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("href")).toBe("/admin/stats");
    expect(screen.getByText("Child")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Toggle Sidebar" })).toBeTruthy();
    expect(screen.getByTestId("sidebar-inset")).toBeTruthy();
  });
});
