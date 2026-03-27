#!/usr/bin/env bash
set -e

REPO="06Felix/randomizer"
BINARY_NAME="randomizer"
VERSION="${1:-latest}"

# Dependencies
for cmd in curl tar grep; do
	command -v "$cmd" >/dev/null || {
		echo "$cmd is required but not installed"
		exit 1
	}
done

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
Linux) TARGET_OS="unknown-linux-gnu" ;;
Darwin) TARGET_OS="apple-darwin" ;;
*)
	echo "Unsupported OS: $OS"
	exit 1
	;;
esac

# Normalize arch
case "$ARCH" in
x86_64) ARCH="x86_64" ;;
arm64 | aarch64) ARCH="aarch64" ;;
*)
	echo "Unsupported architecture: $ARCH"
	exit 1
	;;
esac

TARGET="$ARCH-$TARGET_OS"
EXT="tar.gz"

# Resolve version
if [ "$VERSION" = "latest" ]; then
	VERSION=$(curl -fsSL \
		-H "Accept: application/vnd.github+json" \
		https://api.github.com/repos/$REPO/releases/latest |
		grep tag_name | cut -d '"' -f 4)

	[ -n "$VERSION" ] || {
		echo "Failed to fetch latest version"
		exit 1
	}
fi

FILE="$BINARY_NAME-$VERSION-$TARGET.$EXT"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"

echo "Installing $BINARY_NAME ($VERSION) for $TARGET"
echo "Downloading from $URL..."

TMP_DIR="$(mktemp -d)"
[ -d "$TMP_DIR" ] || {
	echo "Failed to create temp dir"
	exit 1
}
cd "$TMP_DIR"

curl -fLO "$URL" || {
	echo "Download failed"
	exit 1
}

echo "Extracting..."
tar -xzf "$FILE"

BIN_PATH=$(find . -type f -name "$BINARY_NAME" | head -n 1)

[ -n "$BIN_PATH" ] || {
	echo "Binary not found in archive"
	exit 1
}

INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "Installing to $INSTALL_DIR"
mv "$BIN_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo "✅ Installed!"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
	echo "⚠️ Add this to your PATH:"
	echo "export PATH=\"$INSTALL_DIR:\$PATH\""
fi
