"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Dashboard } from "@/components/dashboard";
import { getAuthStatus } from "@/lib/auth-status";
import { authRedirectPathForSetup } from "@/lib/auth-routing";

export default function Home() {
  const router = useRouter();
  const [ready, setReady] = useState(false);

  const handleLogout = useCallback(() => {
    router.replace("/login");
  }, [router]);

  useEffect(() => {
    const load = async () => {
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated) {
          setReady(true);
          return;
        }
        router.replace(authRedirectPathForSetup(status.setup_complete));
      } catch {
        router.replace("/setup");
      }
    };
    void load();
  }, [router]);

  if (!ready) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return <Dashboard onLogout={handleLogout} />;
}
