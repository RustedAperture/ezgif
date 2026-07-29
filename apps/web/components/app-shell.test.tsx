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

describe("AppShell", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.useUser.mockReset();
  });

  it("hides the stats link for an admin without stats access", () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: false,
        permissions: {
          upload_local_images: false,
          view_admin_stats: false,
        },
      },
    });

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Admin" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Stats" })).toBeNull();
  });

  it("shows the stats link for an admin with stats access", () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: false,
        permissions: {
          upload_local_images: false,
          view_admin_stats: true,
        },
      },
    });

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("href")).toBe("/admin/stats");
  });

  it("shows the stats link for a root admin without an explicit stats grant", () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "root-1",
        username: "root",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
        permissions: {
          upload_local_images: false,
          view_admin_stats: false,
        },
      },
    });

    render(
      <AppShell>
        <p>Child</p>
      </AppShell>,
    );

    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("href")).toBe("/admin/stats");
  });
});
