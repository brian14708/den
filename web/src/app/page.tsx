"use client";

import { useEffect, useState } from "react";

export default function Home() {
  const [health, setHealth] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/health")
      .then((r) => r.json())
      .then((d) => setHealth(d.status))
      .catch(() => setHealth("unreachable"));
  }, []);

  return (
    <main className="flex min-h-screen items-center justify-center">
      <div className="space-y-4 text-center">
        <h1 className="text-4xl font-bold tracking-tight">den</h1>
        <p className="text-neutral-400">personal agent hub</p>
        {health !== null && (
          <p className="text-sm text-neutral-500">
            api:{" "}
            <span
              className={health === "ok" ? "text-green-400" : "text-red-400"}
            >
              {health}
            </span>
          </p>
        )}
      </div>
    </main>
  );
}
