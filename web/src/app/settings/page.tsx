"use client";

import Link from "next/link";
import { DeviceLoginQr } from "@/components/device-login-qr";
import { PasskeyList } from "@/components/passkey-list";

export default function SettingsPage() {
  return (
    <main className="mx-auto max-w-lg px-4 py-12">
      <div className="mb-8 flex items-center gap-4">
        <Link
          href="/"
          className="text-muted-foreground hover:text-foreground text-sm"
        >
          &larr; Back
        </Link>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
      </div>

      <section className="mb-10">
        <h2 className="mb-4 text-lg font-semibold">Log In Another Device</h2>
        <DeviceLoginQr />
      </section>

      <section>
        <h2 className="mb-4 text-lg font-semibold">Passkeys</h2>
        <PasskeyList />
      </section>
    </main>
  );
}
