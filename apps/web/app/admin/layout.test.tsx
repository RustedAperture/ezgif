import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import AdminLayout from "@/app/admin/layout";

const mocks = vi.hoisted(() => ({
  usePathname: vi.fn(),
  useUser: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  usePathname: mocks.usePathname,
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

describe("AdminLayout route composition", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  beforeEach(() => {
    vi.stubGlobal("matchMedia", vi.fn().mockImplementation(() => ({
      matches: false,
      media: "",
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })));

    mocks.usePathname.mockReset();
    mocks.useUser.mockReset();
    mocks.usePathname.mockReturnValue("/admin/users");
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
          manage_permissions: false,
          view_admin_stats: false,
        },
      },
    });
  });

  it("renders the admin route with exactly one main landmark", () => {
    render(
      <AdminLayout>
        <p>Admin content</p>
      </AdminLayout>,
    );

    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("link", { name: "Admin" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.getByRole("link", { name: "Users" }).getAttribute("href")).toBe("/admin/users");
    expect(screen.getByRole("link", { name: "Stats" }).getAttribute("href")).toBe("/admin/stats");
    expect(screen.getByText("Admin content")).toBeTruthy();
  });
});
