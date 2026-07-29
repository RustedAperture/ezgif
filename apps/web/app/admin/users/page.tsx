import { AdminUsersTable } from "@/components/admin-users-table";
import { RequireAdmin } from "@/components/require-admin";

export default function AdminUsersPage() {
  return (
    <RequireAdmin>
      <div className="flex min-h-0 flex-1 flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold">Admin users</h1>
          <p className="text-sm text-muted-foreground">Manage roles and permissions for existing users.</p>
        </div>
        <AdminUsersTable />
      </div>
    </RequireAdmin>
  );
}
