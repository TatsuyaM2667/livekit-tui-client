#!/bin/bash
set -e

echo "=================================================="
echo "  LiveKit TUI Client 1-Click Installer"
echo "=================================================="

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    OS_LIKE=$ID_LIKE
else
    echo "Error: Cannot detect Operating System."
    exit 1
fi

echo "[*] Detected OS: $OS"

# 1. Install System Dependencies
echo "[*] Installing system dependencies..."
if [[ "$OS" == "arch" || "$OS_LIKE" == *"arch"* ]]; then
    sudo pacman -Sy --needed --noconfirm base-devel openssl alsa-lib git curl wget unzip zig odin pkgconf glib2 pulseaudio
elif [[ "$OS" == "ubuntu" || "$OS" == "debian" || "$OS_LIKE" == *"debian"* ]]; then
    sudo apt-get update
    sudo apt-get install -y build-essential libssl-dev libasound2-dev libpulse-dev git curl wget unzip pkg-config libglib2.0-dev
elif [[ "$OS" == "fedora" || "$OS" == "almalinux" || "$OS" == "rocky" || "$OS_LIKE" == *"rhel"* ]]; then
    sudo dnf install -y openssl-devel alsa-lib-devel pulseaudio-libs-devel gcc git curl wget unzip pkgconf-pkg-config glib2-devel
elif [[ "$OS" == "opensuse"* || "$OS_LIKE" == *"suse"* ]]; then
    sudo zypper install -y libopenssl-devel alsa-devel gcc git curl wget unzip pkg-config glib2-devel zig libpulse-devel
else
    echo "[!] Unsupported package manager. Please install alsa-lib-devel and openssl-devel manually."
fi

# 2. Install Rust
if ! command -v cargo &> /dev/null; then
    echo "[*] Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "[*] Rust is already installed."
fi

# 3. Install Zig and Odin for non-Arch systems
# (Arch already installed them via pacman)
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

if ! command -v zig &> /dev/null; then
    echo "[*] Zig not found. Downloading static binary..."
    ZIG_VER="0.13.0"
    wget -qO- "https://ziglang.org/download/${ZIG_VER}/zig-linux-x86_64-${ZIG_VER}.tar.xz" | tar -xJ -C /tmp
    cp -r /tmp/zig-linux-x86_64-${ZIG_VER}/* "$INSTALL_DIR/"
    rm -rf /tmp/zig-linux-x86_64-${ZIG_VER}
fi

if ! command -v odin &> /dev/null; then
    echo "[*] Odin not found. Downloading static binary..."

    # Resolve actual download URL from GitHub API
    API_URL="https://api.github.com/repos/odin-lang/Odin/releases/latest"
    ODIN_URL=$(curl -sSfL "$API_URL" 2>/dev/null \
        | grep -o '"browser_download_url": "[^"]*linux[^"]*"' \
        | head -1 | cut -d'"' -f4)

    if [ -z "$ODIN_URL" ]; then
        ODIN_URL=$(curl -sSfL "$API_URL" 2>/dev/null \
            | grep -o '"browser_download_url": "[^"]*ubuntu[^"]*"' \
            | head -1 | cut -d'"' -f4)
    fi

    if [ -z "$ODIN_URL" ]; then
        echo "[!] Could not determine Odin download URL from GitHub API."
        echo "[*] Falling back to dev-2025-04 release..."
        ODIN_URL="https://github.com/odin-lang/Odin/releases/download/dev-2025-04/odin-ubuntu-amd64-dev-2025-04.zip"
    fi

    echo "[*] Downloading Odin from ${ODIN_URL} ..."

    # Detect archive type from URL
    case "$ODIN_URL" in
        *.tar.gz|*.tgz)
            ARCHIVE="/tmp/odin.tar.gz"
            EXTRACT_CMD="tar -xzf"
            ;;
        *.zip)
            ARCHIVE="/tmp/odin.zip"
            EXTRACT_CMD="unzip -q"
            ;;
        *)
            echo "[!] Unknown archive format: $ODIN_URL"
            echo "    Please install Odin manually: https://odin-lang.org/docs/install/"
            exit 1
            ;;
    esac

    if ! wget -qO "$ARCHIVE" "$ODIN_URL"; then
        echo "[!] Failed to download Odin. Please install manually:"
        echo "    https://odin-lang.org/docs/install/"
        exit 1
    fi

    rm -rf /tmp/odin_extracted
    mkdir -p /tmp/odin_extracted

    if [ "$EXTRACT_CMD" = "unzip -q" ]; then
        unzip -q "$ARCHIVE" -d /tmp/odin_extracted
    else
        tar -xzf "$ARCHIVE" -C /tmp/odin_extracted
    fi

    # Handle nested top-level directory (common in Odin releases)
    cd /tmp/odin_extracted
    if [ "$(ls -1 | wc -l)" -eq 1 ] && [ -d "$(ls -1)" ]; then
        cd "$(ls -1)"
    fi

    cp -r ./* "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/odin" 2>/dev/null || true
    cd /
    rm -rf "$ARCHIVE" /tmp/odin_extracted
fi

# Ensure ~/.local/bin is in PATH for the rest of the script
export PATH="$INSTALL_DIR:$PATH"

# 4. Build and Install the Client
echo "[*] Building LiveKit TUI Client..."

# If we are not already inside the project directory, clone it
if [ ! -f "Cargo.toml" ] || ! grep -q "livekit-tui-client" Cargo.toml; then
    echo "[*] Cloning repository..."
    cd /tmp
    rm -rf livekit-tui-client
    git clone https://github.com/TatsuyaM2667/livekit-tui-client.git
    cd livekit-tui-client
fi

echo "[*] Compiling with Cargo..."
cargo install --path .

# 5. Cleanup old binary if it exists
if [ -f "$HOME/.cargo/bin/client" ]; then
    echo "[*] Cleaning up old 'client' binary..."
    rm "$HOME/.cargo/bin/client"
fi

echo "=================================================="
echo "  Installation Complete!"
echo "=================================================="
echo "You can now run the app from anywhere by typing:"
echo "  livekit-tui-client"
echo ""
echo "Note: Make sure ~/.cargo/bin and ~/.local/bin are in your PATH."
