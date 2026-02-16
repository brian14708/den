"use client";

import { useCallback } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { PasskeyList } from "@/components/passkey-list";
import { getUnauthorizedRedirectPath } from "@/lib/auth-routing";

export default function SettingsPage() {
  const router = useRouter();
  const handleUnauthorized = useCallback(() => {
    void (async () => {
      const path = await getUnauthorizedRedirectPath();
      router.replace(path);
    })();
  }, [router]);

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
        <PasskeyList onUnauthorized={handleUnauthorized} />
      </section>
    </main>
  );
}
