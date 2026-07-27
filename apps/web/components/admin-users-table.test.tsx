import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });

  return { promise, resolve };
}

function adminUser(username: string) {
  return {
    id: `${username}-id`,
    username,
    display_name: null,
    role: "user" as const,
    is_root_admin: false,
    identities: [{ provider: "discord", masked_id: "discord:****1234" }],
    permissions: {
      upload_local_images: false,
      view_admin_stats: false,
      manage_permissions: false,
    },
  };
}

describe("AdminUsersTable", () => {
  afterEach(cleanup);

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

  it("keeps the most recently submitted search results when an older request settles later", async () => {
    const initialRequest = deferred<ReturnType<typeof adminUser>[]>();
    const searchRequest = deferred<ReturnType<typeof adminUser>[]>();
    mocks.apiGet
      .mockImplementationOnce(() => initialRequest.promise)
      .mockImplementationOnce(() => searchRequest.promise);

    render(<AdminUsersTable />);

    await waitFor(() => expect(mocks.apiGet).toHaveBeenCalledWith("/api/admin/users"));
    fireEvent.change(screen.getByRole("textbox", { name: "Search users" }), { target: { value: "newer" } });
    fireEvent.submit(screen.getByRole("textbox", { name: "Search users" }).closest("form")!);
    await waitFor(() => expect(mocks.apiGet).toHaveBeenLastCalledWith("/api/admin/users?q=newer"));

    searchRequest.resolve([adminUser("newer result")]);
    expect(await screen.findByText("newer result")).toBeTruthy();

    initialRequest.resolve([adminUser("stale result")]);
    await waitFor(() => expect(screen.queryByText("stale result")).toBeNull());
    expect(screen.getByText("newer result")).toBeTruthy();
  });
});
