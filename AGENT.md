# Project Context for AI Agents

**Project Name:** Vortex Media Server
**Purpose:** Self-hosted media server with a high-performance backend and a premium Android client.

---

## Architecture Overview

The project is a **monorepo** containing both the Backend server and the Android client.

### 1. Backend (`/`)
- **Language:** Rust
- **Framework:** Axum (Web), Tokio (Async Runtime)
- **Database:** SQLite (via SQLx)
- **Architecture:** Modular Monolith
- **Key Modules:**
    - `src/api`: REST API route handlers.
    - `src/core`: Business logic (Scanner, Transocder/Streamer).
    - `src/models`: DB structs and Domain entities.
    - `src/providers`: Metadata fetchers (TMDB).

### 2. Frontend (`/android_app`)
- **Language:** Kotlin
- **Framework:** Jetpack Compose (UI), Hilt (DI), ViewModel
- **Architecture:** MVVM + Clean Architecture principles
- **Theme:** "Glassy" Design System (Translucent/Blur effects).
- **Key Packages:**
    - `ui/screens`: Composable screens (Home, Player, Details).
    - `ui/components`: Reusable UI widgets (GlassyCard, GlassyTopBar).
    - `data/repository`: Data fetching (Retrofit + Room/Preferences).
    - `di`: Hilt Dependency Injection modules.

---

## Directory Map

### Backend (`src/`)
| Path | Description |
| :--- | :--- |
| `main.rs` | Entry point. Sets up DB pool, Axum router, and starts server. |
| `api/` | Route handlers (e.g., `media.rs`, `stream.rs`). Return JSON/Stream. |
| `core/scanner.rs` | Logic for scanning filesystem and populating DB. |
| `providers/tmdb.rs` | TMDB API integration for metadata matching. |
| `models/` | Structs mapping to SQLite tables (e.g., `MediaItem`). |

### Frontend (`android_app/.../org/knp/vortex/`)
| Path | Description |
| :--- | :--- |
| `MainActivity.kt` | App Entry + Navigation Graph (`NavHost`). |
| `ui/theme/` | `Color.kt`, `Theme.kt`. Defines the Dark/Glassy look. |
| `ui/components/` | Core UI building blocks (`GlassyComponents.kt`). |
| `ui/screens/home/` | Main dashboard (`HomeScreen.kt`). |
| `ui/screens/player/` | Video player logic (`PlayerScreen.kt`, ExoPlayer). |

---

## Conventions & Patterns

### Backend (Rust)
- **Error Handling:** Use `AppError` enum (mapped to HTTP status codes).
- **State:** `AppState` struct holds the DB pool and Config. Passed via Axum `State` extractor.
- **Async:** almost all IO is async. await database calls.

### Frontend (Android)
- **UI State:** Each screen has a `ViewModel` exposing a `uiState` (StateFlow).
- **Navigation:** All routes defined in `MainActivity`'s `AppNavigation`.
- **Styling:** *Always* use `GlassyBackground` wrapper for screens. Use `GlassyCard` for containers.
- **Images:** Use `AsyncImage` (Coil) with `shimmerEffect` placeholder.

## Common Tasks

- **Adding a new Screen:**
    1. Create `Screen.kt` and `ViewModel.kt` in `ui/screens/newfeature`.
    2. Add route to `MainActivity.kt`.
    3. Use `GlassyBackground` as root.

- **Adding a new API Endpoint:**
    1. Create handler in `src/api/`.
    2. Register route in `src/api/mod.rs` or `main.rs`.
    3. Ensure `AppState` is added if DB access is needed.

---

**End of Context**
