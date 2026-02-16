"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Login } from "@/components/auth/login";
import { getAuthStatus, setAuthenticatedAuthStatus } from "@/lib/auth-status";
import { authRedirectPathForSetup } from "@/lib/auth-routing";

export default function LoginPage() {
  const router = useRouter();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated && status.user_name) {
          router.replace("/");
          return;
        }
        if (!status.setup_complete) {
          router.replace("/setup");
          return;
        }
        if (!cancelled) setReady(true);
      } catch {
        if (!cancelled) router.replace("/setup");
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [router]);

  const handleComplete = useCallback(
    async (userName: string | null) => {
      if (userName) {
        setAuthenticatedAuthStatus(userName);
        router.replace("/");
        return;
      }
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated && status.user_name) {
          setAuthenticatedAuthStatus(status.user_name);
          router.replace("/");
          return;
        }
        router.replace(authRedirectPathForSetup(status.setup_complete));
      } catch {
        router.replace("/setup");
      }
    },
    [router],
  );

  if (!ready) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return (
    <main className="flex min-h-screen items-center justify-center">
      <Login onComplete={handleComplete} />
    </main>
  );
}
