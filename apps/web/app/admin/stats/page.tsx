import { AdminStatsDashboard } from "@/components/admin-stats-dashboard";
import { AppShell } from "@/components/app-shell";
import { RequireAdminStats } from "@/components/require-admin-stats";

export default function AdminStatsPage() {
  return (
    <AppShell>
      <RequireAdminStats>
        <div className="flex min-h-0 flex-1 flex-col gap-6">
          <div>
            <h1 className="text-2xl font-semibold">Admin stats</h1>
            <p className="text-sm text-muted-foreground">
              Review growth, activity, and storage trends for the app.
            </p>
          </div>
          <AdminStatsDashboard />
        </div>
      </RequireAdminStats>
    </AppShell>
  );
}
