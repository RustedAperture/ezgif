"use client";

import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { apiGet, apiPatch } from "@/lib/api";
import type { AdminPermissions, AdminUser } from "@/lib/types";
import { useUser } from "@/components/user-provider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

type PermissionName = keyof AdminPermissions;

const permissionLabels: Record<PermissionName, string> = {
  upload_local_images: "Upload local images",
  view_admin_stats: "View admin stats",
  manage_permissions: "Manage permissions",
};

type PendingRoleChange = {
  userId: string;
  nextRole: AdminUser["role"];
  previous: AdminUser;
};

export function AdminUsersTable() {
  const { user } = useUser();
  const isRootAdmin = user?.is_root_admin === true;
  const [query, setQuery] = useState("");
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [pendingControls, setPendingControls] = useState<Set<string>>(new Set());
  const [pendingRoleChange, setPendingRoleChange] = useState<PendingRoleChange | null>(null);
  const requestGeneration = useRef(0);

  const loadUsers = useCallback(async (searchQuery: string) => {
    const generation = ++requestGeneration.current;
    setLoading(true);
    setError(false);
    setUsers([]);
    try {
      const search = searchQuery.trim();
      const path = search ? `/api/admin/users?q=${encodeURIComponent(search)}` : "/api/admin/users";
      const response = await apiGet<AdminUser[]>(path);
      if (generation === requestGeneration.current) setUsers(response);
    } catch {
      if (generation === requestGeneration.current) setError(true);
    } finally {
      if (generation === requestGeneration.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    queueMicrotask(() => {
      void loadUsers("");
    });
  }, [loadUsers]);

  function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void loadUsers(query);
  }

  function setControlPending(control: string, pending: boolean) {
    setPendingControls((current) => {
      const next = new Set(current);
      if (pending) next.add(control);
      else next.delete(control);
      return next;
    });
  }

  function updateUserRow(userId: string, updater: (user: AdminUser) => AdminUser) {
    setUsers((current) => current.map((item) => (item.id === userId ? updater(item) : item)));
  }

  function requestRoleChange(target: AdminUser, role: AdminUser["role"]) {
    if (!isRootAdmin || target.is_root_admin || target.role === role) return;

    setPendingRoleChange({
      userId: target.id,
      nextRole: role,
      previous: {
        ...target,
        permissions: { ...target.permissions },
        identities: [...target.identities],
      },
    });
  }

  async function confirmRoleChange() {
    if (!pendingRoleChange) return;

    const { nextRole, previous, userId } = pendingRoleChange;
    const control = `${userId}:role`;
    setControlPending(control, true);
    const snapshot = previous;
    try {
      await apiPatch<{ role: AdminUser["role"] }, void>(`/api/admin/users/${userId}/role`, { role: nextRole });
      updateUserRow(userId, (item) => ({
        ...item,
        role: nextRole,
        permissions: nextRole === "admin"
          ? {
            upload_local_images: true,
            view_admin_stats: true,
            manage_permissions: true,
          }
          : item.permissions,
      }));
      setPendingRoleChange(null);
    } catch {
      updateUserRow(userId, () => snapshot);
      setPendingRoleChange(null);
      toast.error("Could not update the user's role.");
    } finally {
      setControlPending(control, false);
    }
  }

  async function updatePermission(target: AdminUser, permission: PermissionName, enabled: boolean) {
    if (permission === "manage_permissions" && !isRootAdmin) return;

    const control = `${target.id}:${permission}`;
    const previous = target.permissions[permission];
    setControlPending(control, true);
    setUsers((current) => current.map((item) => (
      item.id === target.id
        ? { ...item, permissions: { ...item.permissions, [permission]: enabled } }
        : item
    )));
    try {
      await apiPatch<{ permission: PermissionName; enabled: boolean }, void>(
        `/api/admin/users/${target.id}/permissions`,
        { permission, enabled },
      );
    } catch {
      setUsers((current) => current.map((item) => (
        item.id === target.id
          ? { ...item, permissions: { ...item.permissions, [permission]: previous } }
          : item
      )));
      toast.error("Could not update the user's permission.");
    } finally {
      setControlPending(control, false);
    }
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <form className="flex flex-col gap-2 sm:flex-row" onSubmit={submitSearch}>
        <Input
          aria-label="Search users"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search username or linked identity"
        />
        <Button type="submit" disabled={loading}>Search</Button>
      </form>

      {error && <p role="alert" className="text-sm text-destructive">Could not load users. Try again.</p>}
      {loading ? (
        <p className="text-sm text-muted-foreground">Loading users...</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>User</TableHead>
              <TableHead>Linked identities</TableHead>
              <TableHead>Role</TableHead>
              <TableHead className="text-center">Upload</TableHead>
              <TableHead className="text-center">Stats</TableHead>
              <TableHead className="text-center">Manage permissions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {users.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-muted-foreground">No users found.</TableCell>
              </TableRow>
            ) : users.map((target) => {
              const label = target.username ?? target.display_name ?? "Unnamed user";
              const roleControl = `${target.id}:role`;
              const roleDisabled = !isRootAdmin || target.is_root_admin || pendingControls.has(roleControl);
              return (
                <TableRow key={target.id}>
                  <TableCell>
                    <div className="font-medium">{label}</div>
                    {target.display_name && target.display_name !== target.username && (
                      <div className="text-xs text-muted-foreground">{target.display_name}</div>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-wrap gap-1">
                      {target.identities.map((identity) => (
                        <Badge key={`${identity.provider}:${identity.masked_id}`} variant="secondary">
                          {identity.masked_id}
                        </Badge>
                      ))}
                    </div>
                  </TableCell>
                  <TableCell>
                    <Select
                      value={target.role}
                      disabled={roleDisabled}
                      onValueChange={(value) => requestRoleChange(target, value as AdminUser["role"])}
                    >
                      <SelectTrigger aria-label={`Role for ${label}`}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="user">User</SelectItem>
                        <SelectItem value="admin">Admin</SelectItem>
                      </SelectContent>
                    </Select>
                  </TableCell>
                  {(Object.keys(permissionLabels) as PermissionName[]).map((permission) => {
                    const control = `${target.id}:${permission}`;
                    const disabled = pendingControls.has(control) || (permission === "manage_permissions" && !isRootAdmin);
                    return (
                      <TableCell key={permission} className="!pr-3 text-center">
                        <Checkbox className="mx-auto"
                          aria-label={`${permissionLabels[permission]} for ${label}`}
                          checked={target.permissions[permission]}
                          disabled={disabled}
                          onCheckedChange={(checked) => void updatePermission(target, permission, checked === true)}
                        />
                      </TableCell>
                    );
                  })}
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}

      <AlertDialog open={pendingRoleChange !== null} onOpenChange={(open) => !open && setPendingRoleChange(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Confirm role change</AlertDialogTitle>
            <AlertDialogDescription>
              {pendingRoleChange && (
                <>
                  Change {pendingRoleChange.previous.username ?? pendingRoleChange.previous.display_name ?? "this user"} from{" "}
                  {pendingRoleChange.previous.role} → {pendingRoleChange.nextRole}?
                  {pendingRoleChange.previous.role === "user" && pendingRoleChange.nextRole === "admin"
                    ? " All permissions will be granted."
                    : ""}
                </>
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => void confirmRoleChange()}
              disabled={pendingRoleChange ? pendingControls.has(`${pendingRoleChange.userId}:role`) : false}
            >
              Confirm
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
