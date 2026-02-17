import { createRootRoute, Link, Outlet } from "@tanstack/react-router";

export const Route = createRootRoute({
  component: RootComponent,
  notFoundComponent: NotFoundComponent,
});

function RootComponent() {
  return <Outlet />;
}

function NotFoundComponent() {
  return (
    <main className="flex min-h-screen items-center justify-center px-4">
      <div className="space-y-3 text-center">
        <h1 className="text-2xl font-bold tracking-tight">Not found</h1>
        <p className="text-muted-foreground text-sm">
          This page does not exist.
        </p>
        <Link
          to="/"
          className="text-muted-foreground hover:text-foreground text-sm underline underline-offset-4"
        >
          Back to dashboard
        </Link>
      </div>
    </main>
  );
}
