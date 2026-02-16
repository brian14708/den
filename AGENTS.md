# den

Personal agent hub & dashboard. Rust + Next.js → single binary.

This file is the shared memory for all coding agents. Read it every session. Update it as you learn.

## Stack

- Rust (axum) backend, serves API + embedded frontend
- Next.js 15 static export (React 19, Tailwind v4) via `rust-embed`
- Nix flake: crane (Rust) + pnpm (frontend) → single binary or OCI image

## Commands

```bash
cd web && pnpm install && pnpm build   # build frontend (required before cargo)
cargo run                               # dev server on :3000
nix build                               # release binary at ./result/bin/den
nix build .#oci                         # OCI container image
nix fmt                                 # format everything
```

## Layout

```
src/main.rs        — axum server, router
src/api/mod.rs     — API router (/api/*)
src/api/health.rs  — GET /api/health
src/frontend.rs    — rust-embed static serving + SPA fallback
web/src/app/       — Next.js App Router pages
build.rs           — creates empty web/out/ so rust-embed compiles without frontend
flake.nix          — full build pipeline + dev shell + formatting
```

## Conventions

- Rust edition 2024, TypeScript strict, Tailwind v4 CSS-based config
- UI components: shadcn/ui (new-york style, neutral base color, `@/components/ui`). Add via `pnpm dlx shadcn@latest add <component>`
- `nix fmt` runs rustfmt + nixfmt + prettier (with tailwind plugin)
- API endpoints: create `src/api/foo.rs`, add `mod foo` + route in `src/api/mod.rs`
- Frontend pages: create `web/src/app/foo/page.tsx`
- `rust-embed` reads `web/out/` from disk in debug, embeds in release
- Keep dependencies minimal
- Git: conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`), lowercase, imperative, no period
- Always run lints before committing: `nix fmt`, `cargo clippy`, `cd web && pnpm fmt && pnpm build`

## Environment

| Variable   | Default | Description                      |
|------------|---------|----------------------------------|
| `PORT`     | `3000`  | Server listen port               |
| `RUST_LOG` | (none)  | Tracing filter (e.g. `den=debug`) |

## Learnings

Record architectural decisions, gotchas, and preferences here as they arise.

- `rust-embed` over `include_dir` — supports debug-mode filesystem reads without recompile
- pnpm in nix: use top-level `pkgs.fetchPnpmDeps` + `pkgs.pnpmConfigHook`, not `pnpm_10.fetchDeps` (deprecated); `fetcherVersion = 3` required
- crane `cleanCargoSource` strips non-Rust files — frontend copied via `preBuild` in `buildPackage`
- `build.rs` creates empty `web/out/` so the project compiles even without a frontend build
