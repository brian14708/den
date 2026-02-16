"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Login } from "@/components/auth/login";
import { getAuthStatus, setAuthenticatedAuthStatus } from "@/lib/auth-status";
import { type PasskeyAuthResult, type RedirectRequest } from "@/lib/webauthn";

function buildCanonicalLoginUrl(
  canonicalOrigin: string,
  redirect: RedirectRequest | undefined,
): string {
  const url = new URL("/login", canonicalOrigin);
  if (redirect?.redirectOrigin) {
    url.searchParams.set("redirect_origin", redirect.redirectOrigin);
  }
  if (redirect?.redirectPath) {
    url.searchParams.set("redirect_path", redirect.redirectPath);
  }
  return url.toString();
}

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

export default function LoginPage() {
  const router = useRouter();
  const [ready, setReady] = useState(false);
  const [redirect] = useState<RedirectRequest | undefined>(() =>
    readRedirectFromLocation(),
  );
  useEffect(() => {
    let cancelled = false;
    const redirectForLoad = redirect;

    const load = async () => {
      try {
        const status = await getAuthStatus({ force: true });
        const currentOrigin = window.location.origin;
        if (
          currentOrigin.toLowerCase() !== status.canonical_origin.toLowerCase()
        ) {
          const redirectForCanonical = redirectForLoad ?? {
            redirectOrigin: currentOrigin,
            redirectPath: "/",
          };
          window.location.assign(
            buildCanonicalLoginUrl(
              status.canonical_origin,
              redirectForCanonical,
            ),
          );
          return;
        }
        if (status.authenticated && status.user_name) {
          if (redirectForLoad) {
            try {
              const redirectUrl = await startRedirect(redirectForLoad);
              window.location.assign(redirectUrl);
              return;
            } catch {
              // Fall back to canonical dashboard if redirect fails.
            }
          }
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
  }, [router, redirect]);

  const handleComplete = useCallback(
    async (result: PasskeyAuthResult) => {
      if (result.redirectUrl) {
        window.location.assign(result.redirectUrl);
        return;
      }
      if (result.userName) {
        setAuthenticatedAuthStatus(result.userName);
        if (redirect) {
          try {
            const redirectUrl = await startRedirect(redirect);
            window.location.assign(redirectUrl);
            return;
          } catch {
            // Continue with canonical session if redirect fails.
          }
        }
        router.replace("/");
        return;
      }
      try {
        const status = await getAuthStatus({ force: true });
        if (status.authenticated && status.user_name) {
          setAuthenticatedAuthStatus(status.user_name);
          if (redirect) {
            try {
              const redirectUrl = await startRedirect(redirect);
              window.location.assign(redirectUrl);
              return;
            } catch {
              // Continue with canonical session if redirect fails.
            }
          }
          router.replace("/");
          return;
        }
        router.replace(status.setup_complete ? "/login" : "/setup");
      } catch {
        router.replace("/setup");
      }
    },
    [router, redirect],
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
      <Login redirect={redirect} onComplete={handleComplete} />
    </main>
  );
}
