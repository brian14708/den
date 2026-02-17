import { useCallback } from "react";

import { createFileRoute } from "@tanstack/react-router";

import { Dashboard } from "@/components/dashboard";

export const Route = createFileRoute("/")({
  component: HomeComponent,
});

function HomeComponent() {
  const handleLogout = useCallback(() => window.location.replace("/login"), []);
  return <Dashboard onLogout={handleLogout} />;
}
