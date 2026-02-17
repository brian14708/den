"use client";

import { useRouter } from "next/navigation";
import { Setup } from "@/components/auth/setup";

export default function SetupPage() {
  const router = useRouter();

  return (
    <main className="flex min-h-screen items-center justify-center">
      <Setup
        onComplete={async () => {
          router.replace("/");
        }}
      />
    </main>
  );
}
