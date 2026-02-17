"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Image from "next/image";
import QRCode from "qrcode";
import { Button } from "@/components/ui/button";
import { apiFetch, isUnauthorizedError } from "@/lib/api-fetch";

interface RedirectStartResponse {
  redirect_url?: string;
}

async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const el = document.createElement("textarea");
  el.value = text;
  el.setAttribute("readonly", "");
  el.style.cssText = "position:fixed;top:-9999px;opacity:0";
  document.body.appendChild(el);
  el.focus();
  el.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(el);
  if (!ok) throw new Error("copy failed");
}

async function createRedirectUrl(): Promise<string> {
  const redirectOrigin = window.location.origin;
  const res = await apiFetch("/api/auth/redirect/start", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      redirect_origin: redirectOrigin,
      redirect_path: "/",
    }),
  });
  if (!res.ok) {
    throw new Error("Failed to create login QR");
  }
  const data = (await res.json()) as RedirectStartResponse;
  if (!data.redirect_url) {
    throw new Error("Missing redirect URL");
  }
  return data.redirect_url;
}

export function DeviceLoginQr() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [redirectUrl, setRedirectUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const inFlightRef = useRef(false);

  const generateQr = useCallback(async (resetCopied: boolean) => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    setLoading(true);
    setError(null);
    if (resetCopied) setCopied(false);
    try {
      const url = await createRedirectUrl();
      const dataUrl = await QRCode.toDataURL(url, {
        errorCorrectionLevel: "M",
        margin: 1,
        width: 280,
      });
      setRedirectUrl(url);
      setQrDataUrl(dataUrl);
    } catch (e) {
      if (!isUnauthorizedError(e))
        setError("Failed to generate login QR. Try again.");
    } finally {
      inFlightRef.current = false;
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!qrDataUrl) return;
    const intervalId = window.setInterval(() => {
      void generateQr(false);
    }, 30_000);
    return () => window.clearInterval(intervalId);
  }, [generateQr, qrDataUrl]);

  const handleGenerate = async () => {
    await generateQr(true);
  };

  const handleCopy = async () => {
    if (!redirectUrl) return;
    try {
      await copyText(redirectUrl);
      setError(null);
      setCopied(true);
    } catch {
      setError("Failed to copy login link.");
    }
  };

  return (
    <div className="space-y-4">
      {error && <p className="text-destructive text-sm">{error}</p>}
      <Button variant="outline" onClick={handleGenerate} disabled={loading}>
        {loading
          ? "Generating QR..."
          : qrDataUrl
            ? "Regenerate login QR"
            : "Generate login QR"}
      </Button>
      {qrDataUrl && (
        <div className="space-y-3 rounded-lg border p-4">
          <Image
            src={qrDataUrl}
            alt="Login QR code"
            width={280}
            height={280}
            className="mx-auto rounded-md border bg-white p-2"
          />
          <p className="text-muted-foreground text-sm">
            Scan this from the device you want to sign in. The QR expires in
            about 60 seconds and auto-refreshes every 30 seconds.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="sm" onClick={handleCopy}>
              {copied ? "Copied" : "Copy login link"}
            </Button>
            {redirectUrl && (
              <Button variant="outline" size="sm" asChild>
                <a href={redirectUrl} target="_blank" rel="noreferrer">
                  Open login link
                </a>
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
