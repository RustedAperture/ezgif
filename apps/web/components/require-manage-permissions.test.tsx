import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RequireManagePermissions } from "@/components/require-manage-permissions";

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

describe("RequireManagePermissions", () => {
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
          manage_permissions: true,
          view_admin_stats: false,
        },
      },
    });
  });

  it("shows the protected content for an admin with manage permissions", () => {
    render(
      <RequireManagePermissions>
        <p>Admin users</p>
      </RequireManagePermissions>,
    );

    expect(screen.getByText("Admin users")).toBeTruthy();
    expect(mocks.push).not.toHaveBeenCalled();
  });

  it("shows the protected content for a root admin without an explicit manage grant", () => {
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
          manage_permissions: false,
          view_admin_stats: false,
        },
      },
    });

    render(
      <RequireManagePermissions>
        <p>Admin users</p>
      </RequireManagePermissions>,
    );

    expect(screen.getByText("Admin users")).toBeTruthy();
    expect(mocks.push).not.toHaveBeenCalled();
  });

  it("sends admins without manage access to stats when they can still view admin stats", async () => {
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
          manage_permissions: false,
          view_admin_stats: true,
        },
      },
    });

    render(
      <RequireManagePermissions>
        <p>Admin users</p>
      </RequireManagePermissions>,
    );

    expect(screen.getByText("You do not have permission to view this page.")).toBeTruthy();
    expect(screen.queryByText("Admin users")).toBeNull();
    await waitFor(() => expect(mocks.push).toHaveBeenCalledWith("/admin/stats"));
  });

  it("sends admins without manage or stats access back to the dashboard", async () => {
    mocks.useUser.mockReturnValue({
      loading: false,
      user: {
        id: "admin-3",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: false,
        permissions: {
          upload_local_images: false,
          manage_permissions: false,
          view_admin_stats: false,
        },
      },
    });

    render(
      <RequireManagePermissions>
        <p>Admin users</p>
      </RequireManagePermissions>,
    );

    expect(screen.getByText("You do not have permission to view this page.")).toBeTruthy();
    expect(screen.queryByText("Admin users")).toBeNull();
    await waitFor(() => expect(mocks.push).toHaveBeenCalledWith("/"));
  });
});
