import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AdminUsersTable } from "@/components/admin-users-table";

const mocks = vi.hoisted(() => ({
  apiGet: vi.fn(),
  apiPatch: vi.fn(),
  useUser: vi.fn(),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("@/lib/api", () => ({
  apiGet: mocks.apiGet,
  apiPatch: mocks.apiPatch,
}));

vi.mock("sonner", () => ({
  toast: { error: vi.fn() },
}));

describe("AdminUsersTable", () => {
  beforeEach(() => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: false,
      },
      loading: false,
    });
    mocks.apiGet.mockResolvedValue([
      {
        id: "user-1",
        username: "member",
        display_name: "Member",
        role: "user",
        is_root_admin: false,
        identities: [{ provider: "discord", masked_id: "discord:****1234" }],
        permissions: {
          upload_local_images: false,
          view_admin_stats: false,
          manage_permissions: false,
        },
      },
    ]);
  });

  it("locks only root-only controls for a regular admin", async () => {
    render(<AdminUsersTable />);

    await screen.findByText("member");

    expect(screen.getByText("discord:****1234")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Role for member" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("checkbox", { name: "Upload local images for member" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("checkbox", { name: "View admin stats for member" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("checkbox", { name: "Manage permissions for member" }).hasAttribute("disabled")).toBe(true);
  });
});
