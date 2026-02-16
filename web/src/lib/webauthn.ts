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

function bufferToBase64url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

export async function registerPasskey(
  userName: string,
  passkeyName: string,
): Promise<void> {
  const beginRes = await fetch("/api/auth/register/begin", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ user_name: userName, passkey_name: passkeyName }),
  });
  if (!beginRes.ok) throw new Error("Registration failed to start");
  const { challenge_id, options } = await beginRes.json();

  // Convert base64url strings to ArrayBuffers for the browser API
  const publicKey = {
    ...options.publicKey,
    challenge: base64urlToBuffer(options.publicKey.challenge),
    user: {
      ...options.publicKey.user,
      id: base64urlToBuffer(options.publicKey.user.id),
    },
    excludeCredentials: options.publicKey.excludeCredentials?.map(
      (c: { id: string; type: string }) => ({
        ...c,
        id: base64urlToBuffer(c.id),
      }),
    ),
  };

  const credential = (await navigator.credentials.create({
    publicKey,
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error("Credential creation was cancelled");

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
  const beginRes = await fetch("/api/auth/login/begin", {
    method: "POST",
  });
  if (!beginRes.ok) throw new Error("Login failed to start");
  const { challenge_id, options } = await beginRes.json();

  const publicKey = {
    ...options.publicKey,
    challenge: base64urlToBuffer(options.publicKey.challenge),
    allowCredentials: options.publicKey.allowCredentials?.map(
      (c: { id: string; type: string }) => ({
        ...c,
        id: base64urlToBuffer(c.id),
      }),
    ),
  };

  const credential = (await navigator.credentials.get({
    publicKey,
  })) as PublicKeyCredential | null;
  if (!credential) throw new Error("Authentication was cancelled");

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
