# den

Personal agent hub & dashboard. Rust + Next.js → single binary.

This file is the shared memory for all coding agents. Read it every session. Update it as you learn.

## Stack

- Rust (axum) backend, serves API + embedded frontend
- Next.js 16 static export (React 19, Tailwind v4) via `rust-embed`
- Nix flake: crane (Rust) + pnpm (frontend) → single binary or OCI image

## Commands

```bash
cd web && pnpm install && pnpm build   # build frontend (required before cargo)
cargo run                               # dev server on :3000
nix build                               # release binary at ./result/bin/den
nix build .#oci                         # OCI container image
cargo fmt                               # format Rust
cd web && pnpm fmt                      # format frontend
cd web && pnpm lint                     # eslint frontend
```

## Layout

```
src/main.rs        — axum server, router, WebAuthn + JWT init
src/api/mod.rs     — API router (/api/*)
src/api/health.rs  — GET /api/health
src/api/auth.rs    — passkey auth endpoints (/api/auth/*)
src/auth.rs        — JWT claims, AuthUser/MaybeAuthUser extractors
src/state.rs       — AppState (SqlitePool, Webauthn, JWT secret)
src/frontend.rs    — rust-embed static serving + SPA fallback
migrations/        — sqlx migrations (run automatically on startup)
web/src/app/       — Next.js App Router pages
web/src/lib/       — shared utilities (webauthn browser helpers)
web/src/components/ — React components (auth/, ui/)
build.rs           — creates empty web/out/ so rust-embed compiles without frontend
flake.nix          — full build pipeline + dev shell
```

## Conventions

- Rust edition 2024, TypeScript strict, Tailwind v4 CSS-based config
- UI components: shadcn/ui (new-york style, neutral base color, `@/components/ui`). Add via `pnpm dlx shadcn@latest add <component>`
- Run formatters directly: `cargo fmt` and `cd web && pnpm fmt`
- API endpoints: create `src/api/foo.rs`, add `mod foo` + route in `src/api/mod.rs`
- Frontend pages: create `web/src/app/foo/page.tsx`
- `rust-embed` reads `web/out/` from disk in debug, embeds in release
- Keep dependencies minimal
- Git: conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`), lowercase, imperative, no period
- Always run lints before committing: `cargo fmt`, `cargo clippy`, `cd web && pnpm lint && pnpm fmt && pnpm build`

## Configuration

Runtime config is loaded from `${XDG_CONFIG_HOME:-~/.config}/den/config.toml`.

```toml
port = 3000
rust_log = "info"
rp_id = "localhost"
rp_origin = "http://localhost:3000"
allowed_hosts = []
# Optional: override path; default is ${XDG_DATA_HOME:-$HOME/.local/share}/den/den.db
# database_path = "/path/to/den.db"
```

## Learnings

Record architectural decisions, gotchas, and preferences here as they arise.

- `rust-embed` over `include_dir` — supports debug-mode filesystem reads without recompile
- pnpm in nix: use top-level `pkgs.fetchPnpmDeps` + `pkgs.pnpmConfigHook`, not `pnpm_10.fetchDeps` (deprecated); `fetcherVersion = 3` required
- crane `cleanCargoSource` strips non-Rust files — frontend copied via `preBuild` in `buildPackage`
- `build.rs` creates empty `web/out/` so the project compiles even without a frontend build
- sqlx migrations: add numbered SQL files in `migrations/` (e.g. `0002_widgets.sql`), they run automatically on startup
- nix build uses `SQLX_OFFLINE=true` — after changing queries, run `cargo sqlx prepare` to update `.sqlx/` cache
- Run Rust/JS formatters directly instead of relying on a combined formatter command
- QR device login uses `/api/auth/redirect/start` to mint short-lived links and now accepts canonical `rp_origin` as a valid redirect target
