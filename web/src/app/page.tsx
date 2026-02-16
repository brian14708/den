"use client";

import { useCallback, useEffect, useState } from "react";
import { Setup } from "@/components/auth/setup";
import { Login } from "@/components/auth/login";
import { Dashboard } from "@/components/dashboard";
import {
  type AuthStatus,
  getAuthStatus,
  setAuthenticatedAuthStatus,
  setLoggedOutAuthStatus,
} from "@/lib/auth-status";

type AuthState = "loading" | "setup" | "login" | "authenticated";

export default function Home() {
  const [authState, setAuthState] = useState<AuthState>("loading");
  const [userName, setUserName] = useState("");

  const applyStatus = useCallback((status: AuthStatus) => {
    if (status.authenticated && status.user_name) {
      setUserName(status.user_name);
      setAuthState("authenticated");
      return;
    }
    setUserName("");
    setAuthState(status.setup_complete ? "login" : "setup");
  }, []);

  const checkStatus = useCallback(
    async (force = false) => {
      try {
        const status = await getAuthStatus({ force });
        applyStatus(status);
      } catch {
        setAuthState("setup");
      }
    },
    [applyStatus],
  );

  const handleSetupComplete = useCallback((name: string) => {
    setAuthenticatedAuthStatus(name);
    setUserName(name);
    setAuthState("authenticated");
  }, []);

  const handleLoginComplete = useCallback(
    async (name: string | null) => {
      if (name) {
        setAuthenticatedAuthStatus(name);
        setUserName(name);
        setAuthState("authenticated");
        return;
      }
      await checkStatus(true);
    },
    [checkStatus],
  );

  const handleLogout = useCallback(() => {
    setLoggedOutAuthStatus();
    setUserName("");
    setAuthState("login");
  }, []);

  useEffect(() => {
    void checkStatus(); // eslint-disable-line react-hooks/set-state-in-effect -- fetch auth status on initial mount
  }, [checkStatus]);

  if (authState === "loading") {
    return (
      <main className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </main>
    );
  }

  return (
    <main className="flex min-h-screen items-center justify-center">
      {authState === "setup" && <Setup onComplete={handleSetupComplete} />}
      {authState === "login" && <Login onComplete={handleLoginComplete} />}
      {authState === "authenticated" && (
        <Dashboard userName={userName} onLogout={handleLogout} />
      )}
    </main>
  );
}
