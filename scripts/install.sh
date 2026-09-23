#!/usr/bin/env bash
# ==============================================================================
# Potato CLI — Unix/Linux/macOS Universal Terminal Installer
# Registers `potato` globally in /usr/local/bin or ~/.local/bin
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "🥔 Installing Potato CLI for Unix terminals..."

# Determine installation directory
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

SOURCE_BIN="$SCRIPT_DIR/target/release/potato"

if [ ! -f "$SOURCE_BIN" ]; then
    echo "Compiling native release binary..."
    cargo build --release -p potato-cli --bin potato --manifest-path "$SCRIPT_DIR/Cargo.toml"
fi

cp "$SOURCE_BIN" "$INSTALL_DIR/potato"
chmod +x "$INSTALL_DIR/potato"

echo "✔ Successfully installed to $INSTALL_DIR/potato"
echo "You can now run 'potato --version' from any terminal!"
"$INSTALL_DIR/potato" --version
