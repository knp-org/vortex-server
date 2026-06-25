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

### Server (Linux - Automated Install)
You can easily install or update the Vortex Server on any Linux machine (AMD64 or ARM64) by running our automated install script:
```bash
curl -sSL https://raw.githubusercontent.com/knp-org/vortex-server/main/install.sh | bash
```

### Server (Rust - Manual Build)
1. Install Rust (https://rustup.rs).
2. Run the server:
   ```bash
   cargo run --release
   ```
   *Note: A valid TMDB API Key is required for fetching metadata (configured in app settings).*

### Data Storage
By default, Vortex stores its database and thumbnails in the XDG data directory:
- **Linux:** `~/.local/share/vortex/`
- **macOS:** `~/Library/Application Support/vortex/`

The directory is created automatically on first run — no `sudo` or manual setup required.

To use a custom location, set the `VORTEX_DATA_DIR` environment variable:
```bash
export VORTEX_DATA_DIR=/path/to/custom/data
```

### Running as a Background Service (Systemd)
To ensure Vortex Server automatically starts on boot (like Jellyfin), you can configure it as a systemd service.

1. Build the release binary and move it to your system path:
   ```bash
   cargo build --release
   sudo cp target/release/vortex-server /usr/local/bin/
   ```
2. Copy the provided service file and enable it:
   ```bash
   sudo cp vortex.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now vortex
   ```
   *The data directory (`~/.local/share/vortex/`) will be created automatically by the service. To override the location, add `Environment=VORTEX_DATA_DIR=/your/path` to the `[Service]` section of the service file.*

3. You can check the server status at any time with:
   ```bash
   sudo systemctl status vortex
   ```


### Client (Android)
1. Open `android_app` in Android Studio.
2. Sync Gradle and Run on your device/emulator.
3. The app will connect to the server (ensure same network).

## Contributing
Feel free to open issues or submit PRs!

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). See the [LICENSE](LICENSE) file for details.
