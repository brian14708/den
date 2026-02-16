export interface AuthStatus {
  setup_complete: boolean;
  authenticated: boolean;
  user_name: string | null;
  canonical_origin: string;
}

function currentOrigin(): string {
  return typeof window !== "undefined" ? window.location.origin : "";
}

async function detectSetupCompleteUnauthenticated(): Promise<boolean> {
  // Invalid registration payload lets us distinguish "setup complete" (401) from "not set up" (400).
  const res = await fetch("/api/auth/register/begin", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    cache: "no-store",
    body: JSON.stringify({
      user_name: "",
      passkey_name: "",
    }),
  });
  if (res.status === 401) return true;
  if (res.status === 400) return false;
  throw new Error("Failed to detect setup status");
}

async function fetchAuthStatus(): Promise<AuthStatus> {
  const authCheck = await fetch("/api/auth/passkeys", { cache: "no-store" });
  if (authCheck.status === 401) {
    return {
      setup_complete: await detectSetupCompleteUnauthenticated(),
      authenticated: false,
      user_name: null,
      canonical_origin: currentOrigin(),
    };
  }
  if (!authCheck.ok) throw new Error("Failed to load auth status");

  return {
    setup_complete: true,
    authenticated: true,
    user_name: null,
    canonical_origin: currentOrigin(),
  };
}

export function getCachedAuthStatus(): AuthStatus | null {
  return null;
}

export function setCachedAuthStatus(status: AuthStatus): void {
  void status;
}

export function clearCachedAuthStatus(): void {}

export async function getAuthStatus(options?: {
  force?: boolean;
}): Promise<AuthStatus> {
  void options;
  return fetchAuthStatus();
}

export function setAuthenticatedAuthStatus(userName: string): void {
  void userName;
}

export function setLoggedOutAuthStatus(): void {}
