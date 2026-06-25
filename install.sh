#!/bin/bash
set -euo pipefail

# Repository information
REPO_OWNER="knp-org"  # Assuming from corpus name
REPO_NAME="vortex-server"
GITHUB_API="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
SERVER_PORT=3000

echo "============================================="
echo "   Vortex Server Automated Installer"
echo "============================================="
echo ""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Download a URL to a file, failing loudly on HTTP errors (e.g. 404) instead
# of silently saving an error page. This is the #1 cause of "installed but
# 404" reports: a missing release asset used to be saved as a bogus file.
download() {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1; then
        if ! curl -fSL "$url" -o "$dest"; then
            echo "Error: failed to download $url" >&2
            echo "       The release asset may be missing for tag $LATEST_TAG." >&2
            exit 1
        fi
    else
        if ! wget -O "$dest" "$url"; then
            echo "Error: failed to download $url" >&2
            echo "       The release asset may be missing for tag $LATEST_TAG." >&2
            exit 1
        fi
    fi
}

# Stop any running instance and free the port BEFORE installing, so the new
# service can actually bind. A leftover process holding the port causes the
# systemd unit to crash-loop with "Address already in use" while a stale
# binary keeps answering requests (and serving the wrong static files).
stop_existing() {
    echo "Stopping any existing Vortex instance..."
    sudo systemctl stop vortex_server 2>/dev/null || true

    # Kill any stray process still holding the port (manual launches, etc.)
    if command -v fuser >/dev/null 2>&1; then
        sudo fuser -k "${SERVER_PORT}/tcp" 2>/dev/null || true
    elif command -v ss >/dev/null 2>&1; then
        pids=$(sudo ss -ltnp "sport = :${SERVER_PORT}" 2>/dev/null \
            | grep -oP 'pid=\K[0-9]+' | sort -u || true)
        for pid in $pids; do
            sudo kill "$pid" 2>/dev/null || true
        done
    fi
    sleep 1
}

# Verify the web UI assets actually landed where the server reads them.
verify_static() {
    if [ ! -f /opt/vortex/static/index.html ]; then
        echo "Error: web UI assets are missing (/opt/vortex/static/index.html not found)." >&2
        echo "       The static.tar.gz release asset may be incomplete." >&2
        exit 1
    fi
    echo "Web UI assets verified at /opt/vortex/static/"
}

# Confirm the service is up and serving the web UI (HTTP 200) after install.
verify_running() {
    echo "Verifying the server is responding..."
    for _ in $(seq 1 10); do
        if command -v curl >/dev/null 2>&1; then
            code=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:${SERVER_PORT}/" || echo 000)
        else
            code=000
        fi
        if [ "$code" = "200" ]; then
            echo "Server is up and serving the web UI (HTTP 200)."
            return 0
        fi
        sleep 1
    done
    echo "Warning: server did not return HTTP 200 on http://localhost:${SERVER_PORT}/ (last code: ${code:-unknown})." >&2
    echo "         Check logs with: journalctl -u vortex_server --no-pager | tail -30" >&2
}

# ---------------------------------------------------------------------------
# 1. Detect Architecture
# ---------------------------------------------------------------------------
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64)
        ARCH="amd64"
        ;;
    aarch64|arm64)
        ARCH="arm64"
        ;;
    *)
        echo "Error: Unsupported architecture $ARCH"
        exit 1
        ;;
esac

# 2. Detect OS & Package Manager
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
if [ "$OS" != "linux" ]; then
    echo "Error: This installation script only supports Linux."
    echo "For Windows, please download the .exe directly from the GitHub releases page."
    exit 1
fi

HAS_DPKG=false
if command -v dpkg >/dev/null 2>&1; then
    HAS_DPKG=true
fi

# 3. Fetch latest release information
echo "Fetching latest release information..."
if command -v curl >/dev/null 2>&1; then
    RELEASE_DATA=$(curl -s "$GITHUB_API")
elif command -v wget >/dev/null 2>&1; then
    RELEASE_DATA=$(wget -qO- "$GITHUB_API")
else
    echo "Error: Neither curl nor wget is installed. Please install one of them and try again."
    exit 1
fi

# Extract the latest version tag
LATEST_TAG=$(echo "$RELEASE_DATA" | grep '"tag_name":' | head -n 1 | awk -F'"' '{print $4}' || true)

if [ -z "$LATEST_TAG" ]; then
    echo "Error: Could not determine the latest release version from GitHub."
    echo "Please ensure the repository is public and has published releases."
    exit 1
fi

echo "Latest version found: $LATEST_TAG"

STATIC_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/static.tar.gz"
STATIC_TMP="/tmp/static.tar.gz"

# Free the port / stop the old service before touching files.
stop_existing

# 4. Install
if [ "$HAS_DPKG" = true ]; then
    ASSET_NAME="vortex_server_${ARCH}.deb"
    DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/$ASSET_NAME"
    TMP_FILE="/tmp/$ASSET_NAME"

    echo "Downloading Debian package for $ARCH..."
    download "$DOWNLOAD_URL" "$TMP_FILE"

    echo "Installing Vortex Server via dpkg..."
    sudo dpkg -i "$TMP_FILE"
    rm -f "$TMP_FILE"

    # Ensure web UI assets are complete (deb glob may miss subdirectories).
    echo "Downloading web UI assets..."
    sudo mkdir -p /opt/vortex/static /opt/vortex/data
    download "$STATIC_URL" "$STATIC_TMP"
    sudo tar -xzf "$STATIC_TMP" -C /opt/vortex/
    rm -f "$STATIC_TMP"

    verify_static

    # The .deb ships and enables the systemd unit; restart it now that the
    # static assets are in place and the port has been freed.
    sudo systemctl daemon-reload
    sudo systemctl enable vortex_server 2>/dev/null || true
    sudo systemctl restart vortex_server
else
    ASSET_NAME="vortex_server_linux_${ARCH}"
    DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/$ASSET_NAME"
    INSTALL_DIR="/usr/bin"
    TMP_FILE="/tmp/vortex_server"

    echo "Downloading raw binary for $ARCH..."
    download "$DOWNLOAD_URL" "$TMP_FILE"

    echo "Installing to $INSTALL_DIR/vortex_server..."
    chmod +x "$TMP_FILE"
    sudo mv "$TMP_FILE" "$INSTALL_DIR/vortex_server"

    echo "Setting up systemd service and directories..."
    sudo mkdir -p /opt/vortex/static
    sudo mkdir -p /opt/vortex/data

    echo "Downloading web UI..."
    download "$STATIC_URL" "$STATIC_TMP"
    sudo tar -xzf "$STATIC_TMP" -C /opt/vortex/
    sudo rm -f "$STATIC_TMP"

    verify_static

    # Download the service file
    SERVICE_URL="https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/main/vortex_server.service"
    download "$SERVICE_URL" "/tmp/vortex_server.service"
    sudo mv /tmp/vortex_server.service /etc/systemd/system/vortex_server.service

    sudo systemctl daemon-reload
    sudo systemctl enable vortex_server 2>/dev/null || true
    sudo systemctl restart vortex_server
fi

# 5. Verify the install actually works before declaring success.
verify_running

echo ""
echo "============================================="
echo "   Installation Complete!"
echo "============================================="
echo "Vortex Server has been installed and is now running as a background service."
echo "You can check its status with:"
echo "    sudo systemctl status vortex_server"
echo ""
echo "The Web UI should now be accessible at http://localhost:${SERVER_PORT}"
echo "============================================="
