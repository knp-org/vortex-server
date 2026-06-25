#!/bin/bash
set -e

# Repository information
REPO_OWNER="knp-org"  # Assuming from corpus name
REPO_NAME="vortex-server"
GITHUB_API="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"

echo "============================================="
echo "   Vortex Server Automated Installer"
echo "============================================="
echo ""

# 1. Detect Architecture
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

# 4. Determine which asset to download
if [ "$HAS_DPKG" = true ]; then
    ASSET_NAME="vortex_server_${ARCH}.deb"
    DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/$ASSET_NAME"
    TMP_FILE="/tmp/$ASSET_NAME"
    
    echo "Downloading Debian package for $ARCH..."
    if command -v curl >/dev/null 2>&1; then
        curl -SL "$DOWNLOAD_URL" -o "$TMP_FILE"
    else
        wget -O "$TMP_FILE" "$DOWNLOAD_URL"
    fi
    
    echo "Installing Vortex Server via dpkg..."
    sudo dpkg -i "$TMP_FILE"
    
    # Clean up
    rm "$TMP_FILE"
else
    ASSET_NAME="vortex_server_linux_${ARCH}"
    DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/$ASSET_NAME"
    INSTALL_DIR="/usr/bin"
    TMP_FILE="/tmp/vortex_server"
    
    echo "Downloading raw binary for $ARCH..."
    if command -v curl >/dev/null 2>&1; then
        curl -SL "$DOWNLOAD_URL" -o "$TMP_FILE"
    else
        wget -O "$TMP_FILE" "$DOWNLOAD_URL"
    fi
    
    echo "Installing to $INSTALL_DIR/vortex_server..."
    chmod +x "$TMP_FILE"
    sudo mv "$TMP_FILE" "$INSTALL_DIR/vortex_server"

    echo "Setting up systemd service and directories..."
    sudo mkdir -p /opt/vortex/static
    sudo mkdir -p /opt/vortex/data

    echo "Downloading web UI..."
    STATIC_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_TAG/static.tar.gz"
    STATIC_TMP="/tmp/static.tar.gz"
    if command -v curl >/dev/null 2>&1; then
        curl -SL "$STATIC_URL" -o "$STATIC_TMP"
    else
        wget -O "$STATIC_TMP" "$STATIC_URL"
    fi
    sudo tar -xzf "$STATIC_TMP" -C /opt/vortex/
    sudo rm "$STATIC_TMP"

    # Download the service file
    SERVICE_URL="https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/main/vortex_server.service"
    if command -v curl >/dev/null 2>&1; then
        sudo curl -sL "$SERVICE_URL" -o /etc/systemd/system/vortex_server.service
    else
        sudo wget -qO /etc/systemd/system/vortex_server.service "$SERVICE_URL"
    fi

    sudo systemctl daemon-reload
    sudo systemctl enable --now vortex_server
fi

echo ""
echo "============================================="
echo "   Installation Complete!"
echo "============================================="
echo "Vortex Server has been installed and is now running as a background service."
echo "You can check its status with:"
echo "    sudo systemctl status vortex_server"
echo ""
echo "The Web UI should now be accessible at http://localhost:3000"
echo "============================================="
