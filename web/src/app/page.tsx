"use client";

import { useCallback } from "react";
import { Dashboard } from "@/components/dashboard";

export default function Home() {
  const handleLogout = useCallback(() => window.location.replace("/login"), []);
  return <Dashboard onLogout={handleLogout} />;
}
