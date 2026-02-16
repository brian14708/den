"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Setup } from "@/components/auth/setup";
import { getAuthStatus, setAuthenticatedAuthStatus } from "@/lib/auth-status";

export default function SetupPage() {
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
        if (status.setup_complete) {
          router.replace("/login");
          return;
        }
      } catch {
        // Keep setup available if status check fails.
      }
      if (!cancelled) setReady(true);
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [router]);

  if (!ready) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return (
    <main className="flex min-h-screen items-center justify-center">
      <Setup
        onComplete={(userName) => {
          setAuthenticatedAuthStatus(userName);
          router.replace("/");
        }}
      />
    </main>
  );
}
