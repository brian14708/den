import { getUnauthorizedRedirectPath } from "@/lib/auth-routing";

export class UnauthorizedError extends Error {
  constructor() {
    super("Unauthorized");
    this.name = "UnauthorizedError";
  }
}

interface ApiFetchOptions extends RequestInit {
  redirectOnUnauthorized?: boolean;
}

let redirectPromise: Promise<void> | null = null;

async function redirectForUnauthorized(): Promise<void> {
  if (typeof window === "undefined") return;
  if (!redirectPromise) {
    redirectPromise = getUnauthorizedRedirectPath()
      .then((p) => window.location.replace(p))
      .finally(() => {
        redirectPromise = null;
      });
  }
  await redirectPromise;
}

export async function apiFetch(
  input: RequestInfo | URL,
  init?: ApiFetchOptions,
): Promise<Response> {
  const { redirectOnUnauthorized = true, ...requestInit } = init ?? {};
  const res = await fetch(input, requestInit);
  if (res.status === 401 && redirectOnUnauthorized) {
    await redirectForUnauthorized();
    throw new UnauthorizedError();
  }
  return res;
}

export function isUnauthorizedError(
  error: unknown,
): error is UnauthorizedError {
  return error instanceof UnauthorizedError;
}
