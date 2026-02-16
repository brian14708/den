"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Dashboard } from "@/components/dashboard";
import { getAuthStatus, setLoggedOutAuthStatus } from "@/lib/auth-status";
import { authRedirectPathForSetup } from "@/lib/auth-routing";

export default function Home() {
  const router = useRouter();
  const [userName, setUserName] = useState<string | null>(null);

  const handleLogout = useCallback(() => {
    setLoggedOutAuthStatus();
    router.replace("/login");
  }, [router]);

  useEffect(() => {
    const load = async () => {
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated && status.user_name) {
          setUserName(status.user_name);
          return;
        }
        router.replace(authRedirectPathForSetup(status.setup_complete));
      } catch {
        router.replace("/setup");
      }
    };
    void load();
  }, [router]);

  if (!userName) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return <Dashboard userName={userName} onLogout={handleLogout} />;
}
