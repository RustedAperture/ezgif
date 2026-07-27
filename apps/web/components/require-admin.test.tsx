import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RequireAdmin } from "@/components/require-admin";

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

describe("RequireAdmin", () => {
  beforeEach(() => {
    mocks.push.mockReset();
    mocks.useUser.mockReturnValue({
      loading: false,
      user: {
        id: "user-1",
        username: "member",
        display_name: null,
        avatar_url: null,
        role: "user",
        is_root_admin: false,
      },
    });
  });

  it("denies a non-admin and returns them to the dashboard", async () => {
    render(
      <RequireAdmin>
        <p>Admin users</p>
      </RequireAdmin>,
    );

    expect(screen.getByText("You do not have permission to view this page.")).toBeTruthy();
    expect(screen.queryByText("Admin users")).toBeNull();
    await waitFor(() => expect(mocks.push).toHaveBeenCalledWith("/"));
  });
});
