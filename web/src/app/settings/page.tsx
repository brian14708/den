"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { PasskeyList } from "@/components/passkey-list";
import { getAuthStatus } from "@/lib/auth-status";

export default function SettingsPage() {
  const router = useRouter();
  const [userName, setUserName] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    const applyStatus = (status: {
      authenticated: boolean;
      user_name: string | null;
    }) => {
      if (!status.authenticated || !status.user_name) {
        router.replace("/");
        return;
      }
      setUserName(status.user_name);
      setLoading(false);
    };

    getAuthStatus()
      .then((status) => {
        if (!cancelled) applyStatus(status);
      })
      .catch(() => {
        if (!cancelled) router.replace("/");
      });

    return () => {
      cancelled = true;
    };
  }, [router]);

  if (loading) {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return (
    <main className="mx-auto max-w-lg px-4 py-12">
      <div className="mb-8 flex items-center gap-4">
        <Link
          href="/"
          className="text-muted-foreground hover:text-foreground text-sm"
        >
          &larr; Back
        </Link>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
      </div>

      <section>
        <h2 className="mb-4 text-lg font-semibold">Passkeys</h2>
        {userName && <PasskeyList userName={userName} />}
      </section>
    </main>
  );
}
