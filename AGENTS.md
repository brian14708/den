# den

Personal agent hub & dashboard. Rust + Vite.

This file is the shared memory for all coding agents. Read it every session. Update it as you learn.

## Stack

- Rust (axum) backend, serves API + frontend assets from disk
- Vite SPA (React 19, Tailwind v4, TanStack Router)
- Nix flake: crane (Rust) + pnpm (frontend) → package or OCI image

## Commands

```bash
cd web && pnpm install && pnpm build   # build frontend (required before cargo)
cd web && pnpm dev                    # Vite dev server on :3001 (proxies /api -> :3000)
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
src/config.rs      — config.toml defaults + loading from XDG paths
src/api/mod.rs     — API router (/api/*)
src/api/health.rs  — GET /api/health
src/api/auth.rs    — passkey auth endpoints (/api/register, /api/login, /api/logout, /api/passkeys)
src/auth.rs        — JWT claims, AuthUser/MaybeAuthUser extractors
src/origin.rs      — shared origin/header parsing + allowed host normalization
src/middleware.rs  — cross-cutting HTTP middleware (canonical auth-origin redirects)
src/state.rs       — AppState (SqlitePool, Webauthn, JWT secret)
src/frontend.rs    — filesystem static serving + SPA fallback
migrations/        — sqlx migrations (run automatically on startup)
web/index.html     — SPA entry HTML
web/vite.config.ts — Vite config (+ TanStack Router codegen)
web/src/routes/    — TanStack Router file-based routes
web/src/lib/       — shared utilities (webauthn browser helpers)
web/src/components/ — React components (auth/, ui/)
flake.nix          — full build pipeline + dev shell
```

## Conventions

- Rust edition 2024, TypeScript strict, Tailwind v4 CSS-based config
- UI components: shadcn/ui (new-york style, neutral base color, `@/components/ui`). Add via `pnpm dlx shadcn@latest add <component>`
- Run formatters directly: `cargo fmt` and `cd web && pnpm fmt`
- API endpoints: create `src/api/foo.rs`, add `mod foo` + route in `src/api/mod.rs`
- Frontend pages: create `web/src/routes/foo.tsx`
- Frontend assets served from `DEN_WEB_OUT_DIR` or `$exe/../share/den/web/out`; dev fallback is `./web/out` (SPA fallback serves `index.html` for deep links)
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

- Serve frontend from filesystem: resolve via `DEN_WEB_OUT_DIR`, then `$exe/../share/den/web/out`, then `./web/out`
- Static serving: return `index.html` for unknown non-asset paths so SPA routes work on refresh/deep links; keep 404s for missing assets
- pnpm in nix: use top-level `pkgs.fetchPnpmDeps` + `pkgs.pnpmConfigHook`, not `pnpm_10.fetchDeps` (deprecated); `fetcherVersion = 3` required
- crane `cleanCargoSource` strips non-Rust files — frontend built separately and installed under `$out/share/den/web/out`
- sqlx migrations: add numbered SQL files in `migrations/` (e.g. `0002_widgets.sql`), they run automatically on startup
- nix build uses `SQLX_OFFLINE=true` — after changing queries, run `cargo sqlx prepare` to update `.sqlx/` cache
- Run Rust/JS formatters directly instead of relying on a combined formatter command
- QR device login uses `/api/login/redirect` to mint short-lived links and accepts canonical `rp_origin` as a valid redirect target
- `jsonwebtoken` v10 requires exactly one crypto provider feature; set `features = ["rust_crypto"]` (or `["aws_lc_rs"]`) to avoid runtime `CryptoProvider` panics
- Keep origin/host canonicalization in `src/origin.rs`; reuse it from middleware and auth handlers to avoid drift
- Prefer `AuthUser` extractor on protected handlers over route middleware that injects auth extensions
