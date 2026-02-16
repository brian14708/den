const DEFAULT_WEBAUTHN_TIMEOUT_MS = 60_000;
const MIN_WEBAUTHN_TIMEOUT_MS = 5_000;
const MAX_WEBAUTHN_TIMEOUT_MS = 120_000;

function base64urlToBuffer(base64url: string): ArrayBuffer {
  const base64 = base64url.replace(/-/g, "+").replace(/_/g, "/");
  const pad = base64.length % 4;
  const padded = pad ? base64 + "=".repeat(4 - pad) : base64;
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

function bufferToBase64url(buffer: ArrayBuffer | ArrayBufferView): string {
  const bytes =
    buffer instanceof ArrayBuffer
      ? new Uint8Array(buffer)
      : new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function resolveWebAuthnTimeout(timeout: unknown): number {
  if (typeof timeout !== "number" || !Number.isFinite(timeout)) {
    return DEFAULT_WEBAUTHN_TIMEOUT_MS;
  }
  const rounded = Math.trunc(timeout);
  return Math.max(
    MIN_WEBAUTHN_TIMEOUT_MS,
    Math.min(MAX_WEBAUTHN_TIMEOUT_MS, rounded),
  );
}

function isWebAuthnCompatibilityError(error: unknown): boolean {
  if (error instanceof TypeError) return true;
  return error instanceof DOMException && error.name === "NotSupportedError";
}

async function runWithTimeout<T>(
  run: (signal: AbortSignal | undefined) => Promise<T>,
  timeoutMs: number,
): Promise<T> {
  const controller =
    typeof AbortController === "function" ? new AbortController() : null;

  let timeoutId: number | undefined;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timeoutId = window.setTimeout(() => {
      controller?.abort();
      reject(new Error("Passkey request timed out. Try again."));
    }, timeoutMs);
  });

  try {
    return await Promise.race([run(controller?.signal), timeoutPromise]);
  } finally {
    if (timeoutId !== undefined) {
      window.clearTimeout(timeoutId);
    }
  }
}

function passkeyError(
  error: unknown,
  fallbackMessage: string,
  timeoutMessage: string,
): Error {
  if (error instanceof Error && error.message === timeoutMessage) {
    return error;
  }

  if (error instanceof DOMException) {
    switch (error.name) {
      case "AbortError":
        return new Error(timeoutMessage);
      case "NotAllowedError":
        return new Error("Passkey request was cancelled or timed out.");
      case "InvalidStateError":
        return new Error("This passkey is already registered on this device.");
      case "SecurityError":
        return new Error(
          "Passkey failed security checks. Verify RP_ID and RP_ORIGIN for this domain.",
        );
      case "NotSupportedError":
        return new Error(
          "This browser does not support the requested passkey flow.",
        );
      default:
        return new Error(error.message || fallbackMessage);
    }
  }

  if (error instanceof Error) {
    return new Error(error.message || fallbackMessage);
  }

  return new Error(fallbackMessage);
}

function assertPasskeySupport(): void {
  if (typeof window === "undefined" || !("credentials" in navigator)) {
    throw new Error("This browser does not support passkeys.");
  }
  if (typeof PublicKeyCredential === "undefined") {
    throw new Error("This browser does not support passkeys.");
  }
}

async function createCredential(
  publicKey: PublicKeyCredentialCreationOptions,
  signal: AbortSignal | undefined,
): Promise<PublicKeyCredential | null> {
  const request: CredentialCreationOptions = signal
    ? { publicKey, signal }
    : { publicKey };

  try {
    return (await navigator.credentials.create(
      request,
    )) as PublicKeyCredential | null;
  } catch (error) {
    if (signal && isWebAuthnCompatibilityError(error)) {
      return (await navigator.credentials.create({
        publicKey,
      })) as PublicKeyCredential | null;
    }
    throw error;
  }
}

async function getCredential(
  publicKey: PublicKeyCredentialRequestOptions,
  signal: AbortSignal | undefined,
): Promise<PublicKeyCredential | null> {
  const withMediation: CredentialRequestOptions = signal
    ? { mediation: "required", publicKey, signal }
    : { mediation: "required", publicKey };
  const withoutMediation: CredentialRequestOptions = signal
    ? { publicKey, signal }
    : { publicKey };

  try {
    return (await navigator.credentials.get(
      withMediation,
    )) as PublicKeyCredential | null;
  } catch (error) {
    if (!isWebAuthnCompatibilityError(error)) {
      throw error;
    }
  }

  try {
    return (await navigator.credentials.get(
      withoutMediation,
    )) as PublicKeyCredential | null;
  } catch (error) {
    if (signal && isWebAuthnCompatibilityError(error)) {
      return (await navigator.credentials.get({
        publicKey,
      })) as PublicKeyCredential | null;
    }
    throw error;
  }
}

export async function registerPasskey(
  userName: string,
  passkeyName: string,
): Promise<void> {
  assertPasskeySupport();

  const beginRes = await fetch("/api/auth/register/begin", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ user_name: userName, passkey_name: passkeyName }),
  });
  if (!beginRes.ok) throw new Error("Registration failed to start");
  const { challenge_id, options } = await beginRes.json();

  const timeoutMs = resolveWebAuthnTimeout(options.publicKey.timeout);

  // Convert base64url strings to ArrayBuffers for the browser API.
  const publicKey: PublicKeyCredentialCreationOptions = {
    ...options.publicKey,
    timeout: timeoutMs,
    challenge: base64urlToBuffer(options.publicKey.challenge),
    user: {
      ...options.publicKey.user,
      id: base64urlToBuffer(options.publicKey.user.id),
    },
    excludeCredentials: options.publicKey.excludeCredentials?.map(
      (c: { id: string; type: string; transports?: string[] }) => ({
        ...c,
        id: base64urlToBuffer(c.id),
      }),
    ),
  };

  let credential: PublicKeyCredential | null;
  try {
    credential = (await runWithTimeout(
      (signal) => createCredential(publicKey, signal),
      timeoutMs,
    )) as PublicKeyCredential | null;
  } catch (error) {
    throw passkeyError(
      error,
      "Registration failed",
      "Passkey registration timed out. Try again.",
    );
  }
  if (!credential) throw new Error("Passkey registration was cancelled.");

  const response = credential.response as AuthenticatorAttestationResponse;
  const credentialData = {
    id: credential.id,
    rawId: bufferToBase64url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: bufferToBase64url(response.attestationObject),
      clientDataJSON: bufferToBase64url(response.clientDataJSON),
    },
    extensions: credential.getClientExtensionResults(),
  };

  const completeRes = await fetch("/api/auth/register/complete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ challenge_id, credential: credentialData }),
  });
  if (!completeRes.ok) throw new Error("Registration failed to complete");
}

export async function loginWithPasskey(): Promise<void> {
  assertPasskeySupport();

  const beginRes = await fetch("/api/auth/login/begin", {
    method: "POST",
  });
  if (!beginRes.ok) throw new Error("Login failed to start");
  const { challenge_id, options } = await beginRes.json();

  const timeoutMs = resolveWebAuthnTimeout(options.publicKey.timeout);
  const publicKey: PublicKeyCredentialRequestOptions = {
    ...options.publicKey,
    timeout: timeoutMs,
    challenge: base64urlToBuffer(options.publicKey.challenge),
    allowCredentials: options.publicKey.allowCredentials?.map(
      (c: { id: string; type: string; transports?: string[] }) => ({
        ...c,
        id: base64urlToBuffer(c.id),
      }),
    ),
  };

  let credential: PublicKeyCredential | null;
  try {
    credential = (await runWithTimeout(
      (signal) => getCredential(publicKey, signal),
      timeoutMs,
    )) as PublicKeyCredential | null;
  } catch (error) {
    throw passkeyError(
      error,
      "Login failed",
      "Passkey authentication timed out. Try again.",
    );
  }
  if (!credential) throw new Error("Passkey authentication was cancelled.");

  const response = credential.response as AuthenticatorAssertionResponse;
  const credentialData = {
    id: credential.id,
    rawId: bufferToBase64url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: bufferToBase64url(response.authenticatorData),
      clientDataJSON: bufferToBase64url(response.clientDataJSON),
      signature: bufferToBase64url(response.signature),
      userHandle: response.userHandle
        ? bufferToBase64url(response.userHandle)
        : null,
    },
    extensions: credential.getClientExtensionResults(),
  };

  const completeRes = await fetch("/api/auth/login/complete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ challenge_id, credential: credentialData }),
  });
  if (!completeRes.ok) throw new Error("Login failed to complete");
}
