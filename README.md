# Vortex Media Server

**A Next-Gen Self-Hosted Media Experience.**

Vortex combines a high-performance Rust backend with a stunning, premium Android client to deliver your media with zero compromise.

## Key Features

### Premium User Experience
- **Glassmorphic Design:** A modern, translucent interface built with **Jetpack Compose**.
- **Fluid Animations:** Custom bounce effects, shimmer loading states, and smooth transitions.
- **Immersive Layouts:** Edge-to-edge design with dynamic color extraction.

### High-Performance Backend
- **Rust Core:** Powered by **Axum** and **Tokio** for lightning-fast concurrent stream handling.
- **SQLite Database:** Efficient, local metadata storage using **SQLx**.
- **Automatic Metadata:** Smart matching with **TMDB** to fetch posters, plots, and cast info.

### Advanced Playback
- **ExoPlayer Integration:** Support for HLS streaming and direct file playback.
- **Smart Experience:** "Continue Watching" tracks your progress across devices.
- **Native Fullscreen:** Seamless landscape/portrait transitions with state persistence.

### Security
- **Biometric Lock:** Secure your library with Fingerprint or Face Unlock integration.

## Tech Stack

- **Backend:** Rust, Axum, SQLx, Sqlite, Tokio
- **Android:** Kotlin, Jetpack Compose, Material3, Hilt (DI), Coil (Image Loading)
- **Architecture:** MVVM + Repository Pattern

## Setup

### Server (Rust)
1. Install Rust (https://rustup.rs).
2. Run the server:
   ```bash
   cargo run --release
   ```
   *Note: A valid TMDB API Key is required for fetching metadata (configured in app settings).*

### Client (Android)
1. Open `android_app` in Android Studio.
2. Sync Gradle and Run on your device/emulator.
3. The app will connect to the server (ensure same network).

## Contributing
Feel free to open issues or submit PRs!
