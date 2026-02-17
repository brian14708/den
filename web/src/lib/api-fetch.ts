export class UnauthorizedError extends Error {
  constructor() {
    super("Unauthorized");
    this.name = "UnauthorizedError";
  }
}

export async function apiFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const res = await fetch(input, init);
  if (res.status === 401) {
    if (typeof window !== "undefined") window.location.replace("/login");
    throw new UnauthorizedError();
  }
  return res;
}

export function isUnauthorizedError(
  error: unknown,
): error is UnauthorizedError {
  return error instanceof UnauthorizedError;
}
