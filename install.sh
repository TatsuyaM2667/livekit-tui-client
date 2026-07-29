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
    sudo pacman -Sy --needed --noconfirm base-devel alsa-lib git curl wget unzip zig odin
elif [[ "$OS" == "ubuntu" || "$OS" == "debian" || "$OS_LIKE" == *"debian"* ]]; then
    sudo apt-get update
    sudo apt-get install -y build-essential libasound2-dev git curl wget unzip
elif [[ "$OS" == "fedora" || "$OS" == "almalinux" || "$OS" == "rocky" || "$OS_LIKE" == *"rhel"* ]]; then
    sudo dnf install -y alsa-lib-devel gcc git curl wget unzip
elif [[ "$OS" == "opensuse"* || "$OS_LIKE" == *"suse"* ]]; then
    sudo zypper install -y alsa-devel gcc git curl wget unzip
else
    echo "[!] Unsupported package manager. Please install alsa-lib-devel manually."
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
    # Downloading a recent nightly/dev build of Odin for Linux
    ODIN_URL="https://github.com/odin-lang/Odin/releases/download/dev-2024-05/odin-ubuntu-amd64-dev-2024-05.zip"
    wget -qO /tmp/odin.zip "$ODIN_URL"
    unzip -q /tmp/odin.zip -d /tmp/odin_extracted
    cp -r /tmp/odin_extracted/* "$INSTALL_DIR/"
    rm -rf /tmp/odin.zip /tmp/odin_extracted
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

echo "=================================================="
echo "  Installation Complete!"
echo "=================================================="
echo "You can now run the app from anywhere by typing:"
echo "  client"
echo ""
echo "Note: Make sure ~/.cargo/bin and ~/.local/bin are in your PATH."
