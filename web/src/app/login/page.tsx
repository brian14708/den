"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Login } from "@/components/auth/login";
import { type PasskeyAuthResult, type RedirectRequest } from "@/lib/webauthn";

async function startRedirect(redirect: RedirectRequest): Promise<string> {
  const res = await fetch("/api/auth/redirect/start", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      redirect_origin: redirect.redirectOrigin,
      redirect_path: redirect.redirectPath ?? "/",
    }),
  });
  if (!res.ok) throw new Error("Failed to start redirect");
  const data = (await res.json()) as { redirect_url?: string };
  if (!data.redirect_url) throw new Error("Missing redirect URL");
  return data.redirect_url;
}

function readRedirectFromLocation(): RedirectRequest | undefined {
  if (typeof window === "undefined") return undefined;
  const searchParams = new URLSearchParams(window.location.search);
  const redirectOrigin = searchParams.get("redirect_origin")?.trim();
  if (!redirectOrigin) return undefined;
  const redirectPath = searchParams.get("redirect_path")?.trim();
  return {
    redirectOrigin,
    redirectPath: redirectPath || undefined,
  };
}

async function isSetupComplete(): Promise<boolean> {
  const res = await fetch("/api/auth/register/begin", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    cache: "no-store",
    body: JSON.stringify({ user_name: "", passkey_name: "" }),
  });
  return res.status === 401;
}

export default function LoginPage() {
  const router = useRouter();
  const [ready, setReady] = useState(false);
  const [redirect] = useState<RedirectRequest | undefined>(() =>
    readRedirectFromLocation(),
  );

  useEffect(() => {
    isSetupComplete().then((complete) => {
      if (complete) setReady(true);
      else router.replace("/setup");
    });
  }, [router]);

  const handleComplete = useCallback(
    async (result: PasskeyAuthResult) => {
      if (result.redirectUrl) {
        window.location.assign(result.redirectUrl);
        return;
      }
      if (redirect) {
        try {
          const redirectUrl = await startRedirect(redirect);
          window.location.assign(redirectUrl);
          return;
        } catch {
          // Fall through to home if redirect fails.
        }
      }
      router.replace("/");
    },
    [router, redirect],
  );

  if (!ready) return null;

  return (
    <main className="flex min-h-screen items-center justify-center">
      <Login redirect={redirect} onComplete={handleComplete} />
    </main>
  );
}
