import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RequireAdminStats } from "@/components/require-admin-stats";

const mocks = vi.hoisted(() => ({
  push: vi.fn(),
  useUser: vi.fn(),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: mocks.push }),
}));

describe("RequireAdminStats", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.push.mockReset();
    mocks.useUser.mockReset();
    mocks.useUser.mockReturnValue({
      loading: false,
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
  });

  it("shows the protected content for an admin with stats permission", () => {
    render(
      <RequireAdminStats>
        <p>Admin stats dashboard</p>
      </RequireAdminStats>,
    );

    expect(screen.getByText("Admin stats dashboard")).toBeTruthy();
    expect(mocks.push).not.toHaveBeenCalled();
  });

  it("shows the protected content for a root admin without an explicit stats grant", () => {
    mocks.useUser.mockReturnValue({
      loading: false,
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
      <RequireAdminStats>
        <p>Admin stats dashboard</p>
      </RequireAdminStats>,
    );

    expect(screen.getByText("Admin stats dashboard")).toBeTruthy();
    expect(mocks.push).not.toHaveBeenCalled();
  });

  it("sends unauthenticated users to login", async () => {
    mocks.useUser.mockReturnValue({
      loading: false,
      user: null,
    });

    render(
      <RequireAdminStats>
        <p>Admin stats dashboard</p>
      </RequireAdminStats>,
    );

    expect(screen.getByText("Loading...")).toBeTruthy();
    expect(screen.queryByText("Admin stats dashboard")).toBeNull();
    await waitFor(() => expect(mocks.push).toHaveBeenCalledWith("/login"));
  });

  it("sends admins without stats access back to admin users", async () => {
    mocks.useUser.mockReturnValue({
      loading: false,
      user: {
        id: "admin-2",
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
      <RequireAdminStats>
        <p>Admin stats dashboard</p>
      </RequireAdminStats>,
    );

    expect(screen.getByText("You do not have permission to view this page.")).toBeTruthy();
    expect(screen.queryByText("Admin stats dashboard")).toBeNull();
    await waitFor(() => expect(mocks.push).toHaveBeenCalledWith("/admin/users"));
  });
});
