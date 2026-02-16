"use client";

import { useEffect, useState } from "react";
import { Setup } from "@/components/auth/setup";
import { Login } from "@/components/auth/login";
import { Dashboard } from "@/components/dashboard";

type AuthState = "loading" | "setup" | "login" | "authenticated";

interface StatusResponse {
  setup_complete: boolean;
  authenticated: boolean;
  user_name: string | null;
}

export default function Home() {
  const [authState, setAuthState] = useState<AuthState>("loading");
  const [userName, setUserName] = useState("");

  const checkStatus = async () => {
    try {
      const res = await fetch("/api/auth/status");
      const data: StatusResponse = await res.json();
      if (data.authenticated && data.user_name) {
        setUserName(data.user_name);
        setAuthState("authenticated");
      } else if (data.setup_complete) {
        setAuthState("login");
      } else {
        setAuthState("setup");
      }
    } catch {
      setAuthState("setup");
    }
  };

  useEffect(() => {
    checkStatus(); // eslint-disable-line react-hooks/set-state-in-effect -- fetch on mount
  }, []);

  if (authState === "loading") {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return (
    <main className="flex min-h-screen items-center justify-center">
      {authState === "setup" && <Setup onComplete={checkStatus} />}
      {authState === "login" && <Login onComplete={checkStatus} />}
      {authState === "authenticated" && (
        <Dashboard userName={userName} onLogout={checkStatus} />
      )}
    </main>
  );
}
