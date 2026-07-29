import { AdminAreaLayout } from "@/components/admin-area-layout";
import { AppShell } from "@/components/app-shell";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <AppShell>
      <AdminAreaLayout>{children}</AdminAreaLayout>
    </AppShell>
  );
}
