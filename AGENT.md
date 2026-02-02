# Project Context for AI Agents

**Project Name:** Vortex Media Server
**Purpose:** Self-hosted media server with a high-performance Rust backend and a modern React web client.

---

## Architecture Overview

The project is structured as a **monorepo** (conceptually) containing the Backend server and the Web client.

### 1. Backend (`vortex-server`)
- **Language:** Rust
- **Framework:** Axum (Web), Tokio (Async Runtime)
- **Database:** SQLite (via SQLx)
- **Architecture:** Layered Service Architecture (API -> Service -> Repository/DB)
- **Key Design Pattern:** 
  - **Controllers (Handlers):** Thin layer, parses HTTP/JSON, calls Services.
  - **Services:** Contains all business logic (Transcoding, Scanning, Metadata).
  - **Models:** Pure data structures (DTOs and DB Entities).

### 2. Frontend (`vortex-client`)
- **Language:** TypeScript / React
- **Build Tool:** Vite
- **Styling:** Tailwind CSS + Lucide Icons
- **Architecture:** Feature-based + Service Layer
- **Key Concepts:**
  - **Smart Player:** Detects browser capabilities (`DeviceProfile`) to optimize playback.
  - **Centralized Services:** all API calls reside in `src/services/`.
  - **Centralized Types:** Shared interfaces in `src/types/`.

---

## Backend Directory Map (`vortex-server/src/`)

| Path | Component | Description |
| :--- | :--- | :--- |
| `main.rs` | Entry Point | Sets up Tracing, Database Pool, App State, and Router. |
| `api/` | **API Layer** | |
| `api/handlers/` | Handlers | Business-logic agnostic controllers. Grouped by feature (`media`, `library`, `system`, `transcode`). |
| `api/routes.rs` | Router | Defines all REST endpoints and middleware (Auth, Logging). |
| `services/` | **Service Layer** | **Core Logic resides here.** |
| `services/transcode/`| Transcoding | **Smart Transcoding Engine**. Contains `TranscodeService`, `HlsGenerator` (FFmpeg wrapper), and `DeviceProfile` logic. |
| `services/scanner.rs`| Scanner | Recursive filesystem walker. Syncs files to DB. |
| `services/metadata.rs`| Metadata | logic for fetching/refreshing metadata. |
| `metadata_providers/`| Integration | External API Clients (e.g., `TmdbProvider` for Movie/TV data). |
| `models/` | Data Layer | SQLx structs mapping to SQLite tables (e.g., `Media`, `Series`, `Episode`). |
| `infrastructure/` | Infra | Cross-cutting concerns: `config.rs`, `logging.rs`. |

## Frontend Directory Map (`vortex-client/src/`)

| Path | Description |
| :--- | :--- |
| `pages/` | Page components. Refactored into subfolders (e.g., `settings/` with specific tabs). |
| `services/` | API Service layer. `api.ts` (base wrapper), `libraries.ts`, `settings.ts`. return typed promises. |
| `types/` | **Single Source of Truth** for TypeScript interfaces (`Media`, `DeviceProfile`, `StreamInfo`). |
| `components/` | Reusable UI widgets. |

---

## Key Workflows

### 1. Smart Transcoding & Playback
1. **Frontend**: `Player.tsx` calls `detectCapabilities()` to check codec support (HEVC, AV1, etc.).
2. **Request**: Sends `DeviceProfile` payload to `POST /stream/:id/info`.
3. **Decision**: `TranscodeService` checks if `DeviceProfile` supports the current file's video/audio/container.
   - **Direct Play**: File served via `stream_video` (Range requests).
   - **Direct Stream**: Video copied, Audio transcoded (HLS).
   - **Transcode**: Full re-encode (HLS).

### 2. Metadata Scanning
1. **Trigger**: `/api/v1/scan` calls `Scanner::scan_library`.
2. **Process**: 
   - Walk filesystem.
   - Parse filenames (name, year).
   - `TmdbProvider` searches for match.
   - **Optimization**: Images are stored as remote URLs initially (fast search), downloaded lazily for caching.
   - Save to DB.

---

## Developer Conventions

### Backend (Rust)
- **Dependency Injection**: Services are initialized with `SqlitePool` (passed via Axum State).
- **Error Handling**: Use `AppError` enum (impl `IntoResponse`).
- **Async**: Heavy operations (FFmpeg, Scanning) run on Tokio tasks.
- **Logging**: Use `tracing::info!` / `error!`.

### Frontend (React)
- **State**: Use `useState` for local, API services for data.
- **API**: Never call `fetch` directly in components; use `src/services/`.
- **Types**: Always import from `src/types/`, never define interfaces inline for Domain entities.
