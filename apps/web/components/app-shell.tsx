"use client";

import Link from "next/link";
import { LayoutDashboard, FolderOpen, Search, Download, Shield } from "lucide-react";
import { AccountModal } from "./account-modal";
import { ThemeToggle } from "./theme-toggle";
import { useUser } from "./user-provider";

export function AppShell({ children }: { children: React.ReactNode }) {
  const { user } = useUser();
  const canManagePermissions =
    user?.role === "admin" && (user.permissions.manage_permissions || user.is_root_admin);
  const canViewAdminStats =
    user?.role === "admin" && (user.permissions.view_admin_stats || user.is_root_admin);
  const adminHref = canManagePermissions ? "/admin/users" : canViewAdminStats ? "/admin/stats" : null;
  
  return (
    <div className="h-[100dvh] flex flex-col bg-background text-foreground overflow-hidden">
      <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container mx-auto flex h-14 w-full max-w-5xl items-center justify-between px-4 2xl:w-2/3 2xl:max-w-none">
          <Link href="/" className="font-semibold text-lg hover:opacity-80 transition-opacity">
            memeBucket
          </Link>
          <nav className="flex items-center gap-6 text-sm font-medium">
            <Link href="/" className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors">
              <LayoutDashboard className="w-4 h-4" />
              <span className="hidden sm:inline">Dashboard</span>
            </Link>
            <Link href="/buckets?id=all" className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors">
              <FolderOpen className="w-4 h-4" />
              <span className="hidden sm:inline">Buckets</span>
            </Link>
            <Link href="/search" className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors">
              <Search className="w-4 h-4" />
              <span className="hidden sm:inline">Library</span>
            </Link>
            <Link href="/download" className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors">
              <Download className="w-4 h-4" />
              <span className="hidden sm:inline">Download</span>
            </Link>
            {adminHref && (
              <Link href={adminHref} className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors">
                <Shield className="w-4 h-4" />
                <span className="hidden sm:inline">Admin</span>
              </Link>
            )}
            {user && <AccountModal />}
          </nav>
        </div>
      </header>
      <main className="flex-1 flex flex-col min-h-0 mx-auto w-full max-w-5xl px-4 pt-8 pb-4 overflow-y-auto overflow-x-hidden 2xl:w-2/3 2xl:max-w-none">
        {children}
      </main>
      <footer className="border-t">
        <div className="container mx-auto flex w-full max-w-5xl flex-col gap-3 px-4 py-5 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between 2xl:w-2/3 2xl:max-w-none">
          <p>memeBucket</p>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
            <nav className="flex flex-wrap gap-x-4 gap-y-2">
              <Link href="/terms" className="hover:text-foreground transition-colors">
                Terms
              </Link>
              <Link href="/privacy" className="hover:text-foreground transition-colors">
                Privacy
              </Link>
              <Link href="/changelog" className="hover:text-foreground transition-colors">
                Changelog
              </Link>
              <Link href="/license" className="hover:text-foreground transition-colors">
                License
              </Link>
            </nav>
            <ThemeToggle />
          </div>
        </div>
      </footer>
    </div>
  );
}
