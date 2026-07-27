"use client";

import { useUser } from "@/components/user-provider";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

export function RequireAdmin({ children }: { children: React.ReactNode }) {
  const { user, loading } = useUser();
  const router = useRouter();
  const isAdmin = user?.role === "admin";

  useEffect(() => {
    if (loading) return;
    if (!user) {
      router.push("/login");
    } else if (!isAdmin) {
      router.push("/");
    }
  }, [isAdmin, loading, router, user]);

  if (loading || !user) {
    return <div className="flex min-h-[50vh] h-full items-center justify-center">Loading...</div>;
  }

  if (!isAdmin) {
    return (
      <div className="flex min-h-[50vh] h-full items-center justify-center">
        You do not have permission to view this page.
      </div>
    );
  }

  return <>{children}</>;
}
