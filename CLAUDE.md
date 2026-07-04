# CLAUDE.md — Vortex Server

Guidance for AI agents (and humans) working in **vortex-server**, the Rust backend of the
Vortex self-hosted media server. Read this before editing. It documents the architecture,
the non-negotiable rules, and the core design decisions. (`AGENT.md` is an older, partly
stale sketch — prefer this file where they disagree.)

---

## 1. What this is

A high-performance, self-hosted media server. It scans a filesystem of movies, TV, music,
books and photos; enriches them with metadata from external providers (TMDB/TVDB); and
serves them to clients (web SPA, Android, Tauri desktop) over a versioned REST API with
JWT auth and smart on-the-fly transcoding.

- **Language / runtime:** Rust 2021, async on **Tokio**.
- **Web framework:** **Axum 0.7** (+ `tower`/`tower-http` for CORS, static files, tracing).
- **Database:** **SQLite** via **SQLx 0.7** (compile-time-checked pool, WAL mode, FK on).
- **Media:** FFmpeg (external binary) for transcoding/HLS; `lofty` (audio tags),
  `kamadak-exif` + `imagesize` (photos), `lyrics/` sidecar + lrclib.net for lyrics.
- **Auth:** `argon2` password hashing, `jsonwebtoken` JWT, cookie **or** `Authorization`
  header **or** `?token=` query param.
- **Frontend:** shipped as a prebuilt SPA in `static/` (source lives in the separate
  `vortex-client` repo — do not edit `static/assets/*` by hand).

---

## 2. Architecture — layered, one direction

```
HTTP ─▶ api/routes.rs ─▶ middleware (auth, logging, cors)
             │
             ▼
      api/handlers/*      thin controllers: parse request, call a service, return a DTO
             │
             ▼
       services/*         ALL business logic lives here (scan, transcode, metadata, catalog)
             │
             ▼
       models/db/*        SQLx row structs + queries, one file per DB table
             │
             ▼
          SQLite
```

Dependencies point **down only**. Handlers never contain business logic; services never
touch HTTP types (`axum::*`, status codes); models never call services.

### Directory map (`src/`)

| Path | Role |
| :--- | :--- |
| `main.rs` | Entry point: args (`--version`, `--reset-db`), config, DB pool, router, static/SPA fallback, bind. |
| `db/mod.rs` | `init_db()` — creates the DB, opens the pool (WAL, `foreign_keys=on`), runs migrations. |
| `api/routes.rs` | The **single** route table. Defines `AppState`, public vs. protected routes, CORS. |
| `api/middleware.rs` | `auth_middleware` + `AuthUser` extension (`id`, `role`, `require_admin()`). |
| `api/handlers/*` | Controllers, grouped by feature (`media`, `library`, `playback`, `galleries`, `auth`, …). |
| `api/dtos/{requests,responses}.rs` | Wire types: request bodies/queries and response shapes (`Card`, `*Detail`). |
| `services/*` | Business logic. Each is a struct constructed from a `SqlitePool` (see §4). |
| `services/transcode/` | **Smart transcoding engine** (FFmpeg/HLS, device profiles). **Off-limits — see §3.** |
| `services/catalog_service.rs` | Owns all writes to the media spine + detail + normalized tables. |
| `services/scanner.rs` | Recursive filesystem walker; syncs files → DB via `CatalogService`. |
| `models/db/*` | Row structs + SQL, **one file per table** (see §3). |
| `models/metadata.rs` | `NormalizedMetadata` — the provider-agnostic shape services persist. |
| `metadata_providers/*` | External API clients behind a `MetadataProvider` trait + `registry`. |
| `infrastructure/` | Cross-cutting: `config.rs`, `error.rs` (`AppError`), `logging.rs`, `cache.rs`. |
| `migrations/*.sql` | SQLx migrations, timestamp-ordered. The schema's source of truth. |

---

## 3. Rules — do not violate

1. **Never modify `src/services/transcode/`.** Treat it as read-only. Reading/calling it is
   fine; editing it is not.

2. **Model & handler files are named after their DB table (plural).** `movies.rs`,
   `galleries.rs`, `images.rs`, `tracks.rs` — never a conceptual/feature name that isn't a
   table. `media_info.rs` is the documented exception (ffprobe output, not a table).

3. **The "Images" library type is a photo gallery/album feature** (galleries → images),
   parallel to series → episodes. It is **not** backdrop/poster rendering. Galleries are
   grouping entities (a folder of photos); each image row joins 1:1 to its `media_items` spine
   row and references a gallery.

4. **`static/assets/*` is a build artifact** from the `vortex-client` repo. Do not hand-edit
   it. Frontend changes happen in that repo and are deployed here as a fresh bundle.

5. **The schema lives in `migrations/`.** Change the DB only by adding a new timestamped
   migration (`YYYYMMDDHHMMSS_description.sql`); never edit an already-applied migration or
   the `.db` file. Migrations run automatically at startup.

6. **Handlers stay thin; services stay HTTP-free.** No SQL or business rules in handlers; no
   `axum`/status-code knowledge in services.

7. **Errors go through `AppError`** (`infrastructure/error.rs`). Return `Result<_, AppError>`
   from handlers and services; it implements `IntoResponse` and yields a consistent
   `{code, message, status}` JSON body. Add a variant + code rather than returning raw tuples.

8. **Secrets never get logged.** `AppConfig`'s `Debug` redacts `jwt_secret`; keep it that way.

---

## 4. Design conventions

**Service construction (dependency injection).** Services are lightweight structs built from
a cloned `SqlitePool`, per request:

```rust
let cards = MediaService::new(pool.clone()).list_library(id, &library.library_type).await?;
```

There is no global service registry — the pool *is* the shared state. `AppState { pool,
transcode }` is the only long-lived state; `SqlitePool` is extractable directly via
`FromRef`, so most handlers take `State(pool): State<SqlitePool>`.

**The identity spine.** `media_items` is a thin spine — one row per file-backed item holding
only universal facts (`library_id`, `item_type`, `file_path`, timestamps). Each media type has
its own 1:1 detail table keyed on `item_id` (`movies`, `episodes`, `books`, `tracks`,
`music_videos`, `images`). Grouping entities that are **not** files — `series`/`seasons`,
`artists`/`albums`, `galleries` — live in their own tables with their own auto-increment id
and are referenced by FK. The `media_items.id` is the item id used everywhere downstream.

**Normalized metadata.** Genres/tags/studios/people are lookup tables with join tables
(`item_genres`, `credits`, …), not comma-joined strings. `CatalogService` is the **only**
writer of the spine, detail, and normalized tables — the scanner and refresh handlers go
through it, not raw inline SQL.

**Per-user state.** Playback progress, favorites and preferences are per-user
(`user_media_progress`, `user_favorites`, `user_settings`, all keyed by `user_id`). Server-wide
config stays in the global `settings` table. The caller comes from `Extension<AuthUser>`
injected by `auth_middleware`.

**Metadata providers.** Each provider (TMDB, TVDB) implements the `MetadataProvider` trait and
is listed in `metadata_providers::registry` (the single source of truth). They return data that
services normalize into `NormalizedMetadata` before persisting — provider specifics never leak
into the catalog. Image URLs are stored remote-first and cached lazily.

**API surface.** Everything is versioned under `/api/v1/…`. Routes split into `public_routes`
(login, setup, image/thumbnail fetch) and `protected_routes` (everything else, behind
`auth_middleware`). Per-user endpoints are namespaced `/api/v1/me/…`. Admin-only actions call
`auth_user.require_admin()`.

**Config.** `AppConfig::from_env()` (in `infrastructure/config.rs`), accessed via
`config::config()`. Data (DB + thumbnails) defaults to the XDG data dir
(`~/.local/share/vortex/`); override with `VORTEX_DATA_DIR`. Other env knobs: `VORTEX_STATIC_DIR`,
server port, transcode HW-accel, cache size.

**Long-running work** (FFmpeg, scanning, thumbnailing) runs on Tokio tasks; the transcode
service spawns a background maintenance task on startup to bound cache size.

---

## 5. Build, run, test

```bash
cargo run                 # dev server on :3000 (data in ~/.local/share/vortex/)
cargo run --release       # optimized
cargo run -- --version    # print version and exit
cargo run -- --reset-db   # delete DB + thumbnails + transcode cache (media untouched)
cargo build --release     # release binary at target/release/vortex_server
cargo test                # unit/integration tests (see src/test_support.rs)
cargo clippy              # lint before committing
```

- **A TMDB API key is required** for metadata (configured in app settings, not env).
- **FFmpeg must be on PATH** for transcoding.
- Runs as a systemd service in production (`vortex_server.service`); installer is `install.sh`.

---

## 6. Before you commit

- `cargo build` and `cargo clippy` clean.
- New DB changes are a **new migration**, not an edit to an old one or the `.db`.
- New model/handler files are named after their table (§3.2).
- No business logic leaked into handlers; no HTTP types leaked into services.
- Nothing under `services/transcode/` was touched.
- New endpoints registered in `api/routes.rs` and, if user-facing, behind `auth_middleware`.
