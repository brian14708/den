"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";

interface DashboardProps {
  userName: string;
  onLogout: () => void;
}

export function Dashboard({ userName, onLogout }: DashboardProps) {
  const [health, setHealth] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/health")
      .then((r) => r.json())
      .then((d) => setHealth(d.status))
      .catch(() => setHealth("unreachable"));
  }, []);

  const handleLogout = async () => {
    await fetch("/api/auth/logout", { method: "POST" });
    onLogout();
  };

  return (
    <main className="flex min-h-screen items-center justify-center">
      <div className="space-y-4 text-center">
        <h1 className="text-4xl font-bold tracking-tight">den</h1>
        <p className="text-muted-foreground">welcome, {userName}</p>
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
        <Button variant="outline" onClick={handleLogout}>
          Sign out
        </Button>
      </div>
    </main>
  );
}
