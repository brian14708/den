"use client";

import { useCallback, useEffect, useState } from "react";
import { Dashboard } from "@/components/dashboard";
import { getAuthStatus } from "@/lib/auth-status";
import { authRedirectPathForSetup } from "@/lib/auth-routing";

export default function Home() {
  const [ready, setReady] = useState(false);

  const handleLogout = useCallback(() => {
    window.location.replace("/login");
  }, []);

  useEffect(() => {
    const load = async () => {
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated) {
          setReady(true);
          return;
        }
        window.location.replace(authRedirectPathForSetup(status.setup_complete));
      } catch {
        window.location.replace("/setup");
      }
    };
    void load();
  }, []);

  if (!ready) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return <Dashboard onLogout={handleLogout} />;
}
