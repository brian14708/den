import { createFileRoute, useNavigate } from "@tanstack/react-router";

import { Setup } from "@/components/auth/setup";

export const Route = createFileRoute("/setup")({
  component: SetupRouteComponent,
});

function SetupRouteComponent() {
  const navigate = useNavigate();

  return (
    <main className="flex min-h-screen items-center justify-center">
      <Setup
        onComplete={async () => {
          navigate({ to: "/", replace: true });
        }}
      />
    </main>
  );
}
