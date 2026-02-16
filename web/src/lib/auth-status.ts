export interface AuthStatus {
  setup_complete: boolean;
  authenticated: boolean;
  user_name: string | null;
  canonical_origin: string;
}

interface CachedAuthStatus {
  value: AuthStatus;
  updated_at: number;
}

const STORAGE_KEY = "den.auth.status.v1";

let memoryCache: CachedAuthStatus | null = null;
let inflight: Promise<AuthStatus> | null = null;

function currentOrigin(): string {
  return typeof window !== "undefined" ? window.location.origin : "";
}

function isAuthStatus(value: unknown): value is AuthStatus {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<AuthStatus>;
  return (
    typeof candidate.setup_complete === "boolean" &&
    typeof candidate.authenticated === "boolean" &&
    (typeof candidate.user_name === "string" || candidate.user_name === null) &&
    typeof candidate.canonical_origin === "string"
  );
}

function readStorage(): CachedAuthStatus | null {
  if (typeof window === "undefined") return null;
  const raw = window.sessionStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<CachedAuthStatus>;
    if (
      parsed &&
      typeof parsed.updated_at === "number" &&
      isAuthStatus(parsed.value)
    ) {
      return {
        value: parsed.value,
        updated_at: parsed.updated_at,
      };
    }
  } catch {
    return null;
  }
  return null;
}

function writeStorage(entry: CachedAuthStatus): void {
  if (typeof window === "undefined") return;
  window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(entry));
}

function removeStorage(): void {
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(STORAGE_KEY);
}

function setCachedAuthStatusInternal(status: AuthStatus): void {
  const entry: CachedAuthStatus = {
    value: status,
    updated_at: Date.now(),
  };
  memoryCache = entry;
  writeStorage(entry);
}

export function getCachedAuthStatus(): AuthStatus | null {
  if (memoryCache) return memoryCache.value;
  const cached = readStorage();
  if (!cached) return null;
  memoryCache = cached;
  return cached.value;
}

export function setCachedAuthStatus(status: AuthStatus): void {
  setCachedAuthStatusInternal(status);
}

export function clearCachedAuthStatus(): void {
  inflight = null;
  memoryCache = null;
  removeStorage();
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
    const status: AuthStatus = {
      setup_complete: await detectSetupCompleteUnauthenticated(),
      authenticated: false,
      user_name: null,
      canonical_origin: currentOrigin(),
    };
    setCachedAuthStatusInternal(status);
    return status;
  }
  if (!authCheck.ok) throw new Error("Failed to load auth status");

  const status: AuthStatus = {
    setup_complete: true,
    authenticated: true,
    user_name: getCachedAuthStatus()?.user_name ?? null,
    canonical_origin: currentOrigin(),
  };
  setCachedAuthStatusInternal(status);
  return status;
}

export async function getAuthStatus(options?: {
  force?: boolean;
}): Promise<AuthStatus> {
  const force = options?.force ?? false;

  if (!force) {
    const cached = getCachedAuthStatus();
    if (cached) return cached;
    if (inflight) return inflight;
  }

  inflight = fetchAuthStatus().finally(() => {
    inflight = null;
  });

  return inflight;
}

export function setAuthenticatedAuthStatus(userName: string): void {
  const current = getCachedAuthStatus();
  const canonicalOrigin = current?.canonical_origin ?? currentOrigin();
  setCachedAuthStatusInternal({
    setup_complete: current?.setup_complete ?? true,
    authenticated: true,
    user_name: userName,
    canonical_origin: canonicalOrigin,
  });
}

export function setLoggedOutAuthStatus(): void {
  const current = getCachedAuthStatus();
  const canonicalOrigin = current?.canonical_origin ?? currentOrigin();
  setCachedAuthStatusInternal({
    setup_complete: current?.setup_complete ?? true,
    authenticated: false,
    user_name: null,
    canonical_origin: canonicalOrigin,
  });
}
