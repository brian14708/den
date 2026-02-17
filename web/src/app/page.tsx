"use client";

import { useCallback, useEffect, useState } from "react";
import { Dashboard } from "@/components/dashboard";
import { getAuthStatus } from "@/lib/auth-status";
import { authRedirectPathForSetup } from "@/lib/auth-routing";

export default function Home() {
  const [ready, setReady] = useState(false);

  const handleLogout = useCallback(() => window.location.replace("/login"), []);

  useEffect(() => {
    getAuthStatus({ force: true })
      .then((s) =>
        s.authenticated
          ? setReady(true)
          : window.location.replace(authRedirectPathForSetup(s.setup_complete)),
      )
      .catch(() => window.location.replace("/setup"));
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
