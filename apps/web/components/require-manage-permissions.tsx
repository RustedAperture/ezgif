"use client";

import { useUser } from "@/components/user-provider";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

export function RequireManagePermissions({ children }: { children: React.ReactNode }) {
  const { user, loading } = useUser();
  const router = useRouter();
  const isAdmin = user?.role === "admin";
  const canManagePermissions =
    isAdmin && (user.is_root_admin || user.permissions.manage_permissions);
  const canViewAdminStats =
    isAdmin && (user.is_root_admin || user.permissions.view_admin_stats);
  const fallbackRoute = canViewAdminStats ? "/admin/stats" : "/";

  useEffect(() => {
    if (loading) return;
    if (!user) {
      router.push("/login");
    } else if (!isAdmin) {
      router.push("/");
    } else if (!canManagePermissions) {
      router.push(fallbackRoute);
    }
  }, [canManagePermissions, fallbackRoute, isAdmin, loading, router, user]);

  if (loading || !user) {
    return <div className="flex min-h-[50vh] h-full items-center justify-center">Loading...</div>;
  }

  if (!isAdmin || !canManagePermissions) {
    return (
      <div className="flex min-h-[50vh] h-full items-center justify-center">
        You do not have permission to view this page.
      </div>
    );
  }

  return <>{children}</>;
}
