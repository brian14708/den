import { getAuthStatus } from "@/lib/auth-status";

export type AuthRedirectPath = "/login" | "/setup";

export function authRedirectPathForSetup(
  setupComplete: boolean,
): AuthRedirectPath {
  return setupComplete ? "/login" : "/setup";
}

export async function getUnauthorizedRedirectPath(): Promise<AuthRedirectPath> {
  try {
    const status = await getAuthStatus({ force: true });
    return authRedirectPathForSetup(status.setup_complete);
  } catch {
    return "/setup";
  }
}
