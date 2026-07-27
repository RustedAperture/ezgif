import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AdminUsersTable } from "@/components/admin-users-table";

const mocks = vi.hoisted(() => ({
  apiGet: vi.fn(),
  apiPatch: vi.fn(),
  useUser: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("@/lib/api", () => ({
  apiGet: mocks.apiGet,
  apiPatch: mocks.apiPatch,
}));

vi.mock("@/components/ui/select", async () => {
  const React = await import("react");

  type SelectContextValue = {
    value: string;
    disabled?: boolean;
    onValueChange?: (value: string) => void;
  };

  const SelectContext = React.createContext<SelectContextValue | null>(null);

  return {
    Select: ({
      value,
      disabled,
      onValueChange,
      children,
    }: {
      value: string;
      disabled?: boolean;
      onValueChange?: (value: string) => void;
      children: React.ReactNode;
    }) => (
      <SelectContext.Provider value={{ value, disabled, onValueChange }}>
        <div>{children}</div>
      </SelectContext.Provider>
    ),
    SelectTrigger: ({ children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => {
      const context = React.useContext(SelectContext)!;
      return (
        <button role="combobox" disabled={context.disabled} {...props}>
          {children}
        </button>
      );
    },
    SelectValue: () => {
      const context = React.useContext(SelectContext)!;
      return <span>{context.value === "admin" ? "Admin" : "User"}</span>;
    },
    SelectContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => {
      const context = React.useContext(SelectContext)!;
      return (
        <button role="option" type="button" onClick={() => context.onValueChange?.(value)}>
          {children}
        </button>
      );
    },
  };
});

vi.mock("@/components/ui/alert-dialog", async () => {
  const React = await import("react");

  type AlertDialogContextValue = {
    onOpenChange?: (open: boolean) => void;
  };

  const AlertDialogContext = React.createContext<AlertDialogContextValue | null>(null);

  return {
    AlertDialog: ({
      open,
      onOpenChange,
      children,
    }: {
      open: boolean;
      onOpenChange?: (open: boolean) => void;
      children: React.ReactNode;
    }) => (
      <AlertDialogContext.Provider value={{ onOpenChange }}>
        {open ? <div>{children}</div> : null}
      </AlertDialogContext.Provider>
    ),
    AlertDialogContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    AlertDialogHeader: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    AlertDialogTitle: ({ children }: { children: React.ReactNode }) => <h2>{children}</h2>,
    AlertDialogDescription: ({ children }: { children: React.ReactNode }) => <p>{children}</p>,
    AlertDialogFooter: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    AlertDialogAction: ({
      children,
      onClick,
      disabled,
    }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
      <button type="button" onClick={onClick} disabled={disabled}>
        {children}
      </button>
    ),
    AlertDialogCancel: ({ children, disabled }: { children: React.ReactNode; disabled?: boolean }) => {
      const context = React.useContext(AlertDialogContext);
      return (
        <button type="button" disabled={disabled} onClick={() => context?.onOpenChange?.(false)}>
          {children}
        </button>
      );
    },
  };
});

vi.mock("sonner", () => ({
  toast: { error: mocks.toastError },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
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

async function chooseRole(label: string, role: "user" | "admin") {
  fireEvent.click(screen.getByRole("combobox", { name: `Role for ${label}` }));
  fireEvent.click(await screen.findByRole("option", { name: role === "admin" ? "Admin" : "User" }));
}

function isDisabled(element: HTMLElement) {
  return element.hasAttribute("disabled") || element.getAttribute("aria-disabled") === "true";
}

describe("AdminUsersTable", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.toastError.mockReset();
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
    expect(isDisabled(screen.getByRole("combobox", { name: "Role for member" }))).toBe(true);
    expect(isDisabled(screen.getByRole("checkbox", { name: "Upload local images for member" }))).toBe(false);
    expect(isDisabled(screen.getByRole("checkbox", { name: "View admin stats for member" }))).toBe(false);
    expect(isDisabled(screen.getByRole("checkbox", { name: "Manage permissions for member" }))).toBe(true);
  });

  it("centers permission headings and checkbox cells", async () => {
    render(<AdminUsersTable />);

    await screen.findByText("member");

    for (const heading of ["Upload", "Stats", "Manage permissions"]) {
      expect(screen.getByRole("columnheader", { name: heading }).className).toContain("text-center");
    }

    for (const permission of ["Upload local images", "View admin stats", "Manage permissions"]) {
      const checkbox = screen.getByRole("checkbox", { name: permission + " for member" });
      expect(checkbox.className).toContain("mx-auto");
      expect(checkbox.closest("td")?.className).toContain("text-center");
      expect(checkbox.closest("td")?.className).toContain("!pr-3");
    }
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

  it("clears prior results when the latest submitted search fails", async () => {
    const initialRequest = deferred<ReturnType<typeof adminUser>[]>();
    const searchRequest = deferred<ReturnType<typeof adminUser>[]>();
    mocks.apiGet
      .mockImplementationOnce(() => initialRequest.promise)
      .mockImplementationOnce(() => searchRequest.promise);

    render(<AdminUsersTable />);

    await waitFor(() => expect(mocks.apiGet).toHaveBeenCalledWith("/api/admin/users"));
    initialRequest.resolve([adminUser("prior result")]);
    expect(await screen.findByText("prior result")).toBeTruthy();

    fireEvent.change(screen.getByRole("textbox", { name: "Search users" }), { target: { value: "missing" } });
    fireEvent.submit(screen.getByRole("textbox", { name: "Search users" }).closest("form")!);
    await waitFor(() => expect(mocks.apiGet).toHaveBeenLastCalledWith("/api/admin/users?q=missing"));

    searchRequest.reject(new Error("request failed"));

    expect((await screen.findByRole("alert")).textContent).toBe("Could not load users. Try again.");
    await waitFor(() => expect(screen.queryByText("prior result")).toBeNull());
    expect(screen.getByText("No users found.")).toBeTruthy();
  });

  it("opens confirmation before changing a role and does not call the API immediately", async () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
      },
      loading: false,
    });

    render(<AdminUsersTable />);

    await screen.findByText("member");
    await chooseRole("member", "admin");

    expect(await screen.findByText("Confirm role change")).toBeTruthy();
    expect(screen.getByText(/change member from user → admin\?/i)).toBeTruthy();
    expect(screen.getByText(/all permissions will be granted/i)).toBeTruthy();
    expect(mocks.apiPatch).not.toHaveBeenCalled();
  });

  it("restores the original role and permissions when the confirmation is cancelled", async () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
      },
      loading: false,
    });

    render(<AdminUsersTable />);

    await screen.findByText("member");
    await chooseRole("member", "admin");
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByText("Confirm role change")).toBeNull());
    expect(screen.getByRole("combobox", { name: "Role for member" }).textContent).toContain("User");
    expect(screen.getByRole("checkbox", { name: "Upload local images for member" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox", { name: "View admin stats for member" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox", { name: "Manage permissions for member" }).getAttribute("aria-checked")).toBe("false");
    expect(mocks.apiPatch).not.toHaveBeenCalled();
  });

  it("confirms user to admin through the role endpoint and synchronizes all permissions locally", async () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
      },
      loading: false,
    });
    mocks.apiPatch.mockResolvedValue(undefined);

    render(<AdminUsersTable />);

    await screen.findByText("member");
    await chooseRole("member", "admin");
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(mocks.apiPatch).toHaveBeenCalledWith("/api/admin/users/user-1/role", { role: "admin" }));
    expect(mocks.apiPatch).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("combobox", { name: "Role for member" }).textContent).toContain("Admin");
    expect(screen.getByRole("checkbox", { name: "Upload local images for member" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("checkbox", { name: "View admin stats for member" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("checkbox", { name: "Manage permissions for member" }).getAttribute("aria-checked")).toBe("true");
  });

  it("restores the previous role and permissions when the confirmed role request fails", async () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
      },
      loading: false,
    });
    mocks.apiPatch.mockRejectedValue(new Error("role failed"));

    render(<AdminUsersTable />);

    await screen.findByText("member");
    await chooseRole("member", "admin");
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(mocks.apiPatch).toHaveBeenCalledWith("/api/admin/users/user-1/role", { role: "admin" }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Role for member" }).textContent).toContain("User"));
    expect(screen.getByRole("checkbox", { name: "Upload local images for member" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox", { name: "View admin stats for member" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox", { name: "Manage permissions for member" }).getAttribute("aria-checked")).toBe("false");
    expect(mocks.toastError).toHaveBeenCalledWith("Could not update the user's role.");
  });

  it("keeps the confirmation open while the role request is pending", async () => {
    mocks.useUser.mockReturnValue({
      user: {
        id: "admin-1",
        username: "admin",
        display_name: null,
        avatar_url: null,
        role: "admin",
        is_root_admin: true,
      },
      loading: false,
    });
    const roleRequest = deferred<void>();
    mocks.apiPatch.mockReturnValue(roleRequest.promise);

    render(<AdminUsersTable />);

    await screen.findByText("member");
    await chooseRole("member", "admin");
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));

    await waitFor(() => expect(mocks.apiPatch).toHaveBeenCalledWith("/api/admin/users/user-1/role", { role: "admin" }));
    expect(isDisabled(screen.getByRole("button", { name: "Cancel" }))).toBe(true);
    expect(screen.getByText("Confirm role change")).toBeTruthy();

    roleRequest.resolve(undefined);
    await waitFor(() => expect(screen.queryByText("Confirm role change")).toBeNull());
  });
});
