"use client";

import Link from "next/link";
import { BarChart3, ShieldCheck } from "lucide-react";
import { usePathname } from "next/navigation";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { useUser } from "@/components/user-provider";

type AdminSection = {
  href: "/admin/users" | "/admin/stats";
  label: "Users" | "Stats";
  icon: typeof ShieldCheck;
};

export function AdminAreaLayout({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { user } = useUser();

  const canManagePermissions =
    user?.role === "admin" && (user.permissions.manage_permissions || user.is_root_admin);
  const canViewAdminStats =
    user?.role === "admin" && (user.permissions.view_admin_stats || user.is_root_admin);

  const sections: AdminSection[] = [
    ...(canManagePermissions
      ? [{ href: "/admin/users" as const, label: "Users" as const, icon: ShieldCheck }]
      : []),
    ...(canViewAdminStats
      ? [{ href: "/admin/stats" as const, label: "Stats" as const, icon: BarChart3 }]
      : []),
  ];

  return (
    <SidebarProvider className="flex min-h-0 flex-1 w-full overflow-hidden rounded-xl border bg-muted/30 relative">
      <Sidebar className="absolute h-full bg-transparent border-r-0 hidden md:flex" collapsible="offcanvas" variant="inset">
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupLabel>Admin</SidebarGroupLabel>
            <SidebarMenu>
              {sections.map((section) => {
                const Icon = section.icon;

                return (
                  <SidebarMenuItem key={section.href}>
                    <SidebarMenuButton
                      render={<Link href={section.href} />}
                      isActive={pathname === section.href}
                    >
                      <Icon />
                      <span>{section.label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset className="flex min-h-0 flex-1 flex-col m-2 overflow-hidden rounded-xl border bg-background shadow-sm">
        <header className="flex h-14 shrink-0 items-center border-b px-4 lg:px-6">
          <div className="flex items-center gap-2">
            <SidebarTrigger className="h-8 w-8 -ml-2 text-muted-foreground" />
            <span className="text-sm font-medium text-muted-foreground">Admin</span>
          </div>
        </header>
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4 lg:p-6">
          {children}
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
