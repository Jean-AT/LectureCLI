#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
LECTURE_BIN="$CARGO_BIN_DIR/lecture"
LOCAL_BIN="$HOME/.local/bin"
WHISPER_CPP_DIR="${WHISPER_CPP_DIR:-$ROOT_DIR/../whisper.cpp}"
WHISPER_BUILD_DIR="$WHISPER_CPP_DIR/build"
WHISPER_BIN="$WHISPER_BUILD_DIR/bin/whisper-cli"
WHISPER_MODEL_DIR="$WHISPER_CPP_DIR/models"
WHISPER_MODEL="$WHISPER_MODEL_DIR/ggml-base.bin"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but not installed."
  exit 1
fi

if [ ! -d "$WHISPER_CPP_DIR" ]; then
  echo "whisper.cpp not found at: $WHISPER_CPP_DIR"
  echo "Set WHISPER_CPP_DIR to the clone location and run again."
  exit 1
fi

echo "Installing lecture..."
cargo install --path "$ROOT_DIR" --locked

if [ ! -x "$WHISPER_BIN" ]; then
  echo "Building whisper.cpp..."
  cmake -S "$WHISPER_CPP_DIR" -B "$WHISPER_BUILD_DIR" -DWHISPER_BUILD_TESTS=OFF
  cmake --build "$WHISPER_BUILD_DIR" -j
fi

if [ ! -f "$WHISPER_MODEL" ]; then
  echo "Downloading Whisper model..."
  bash "$WHISPER_CPP_DIR/models/download-ggml-model.sh" base "$WHISPER_MODEL_DIR"
fi

mkdir -p "$LOCAL_BIN"
cat > "$LOCAL_BIN/lecture" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export LECTURE_WHISPER_CPP_DIR="$WHISPER_CPP_DIR"
export LECTURE_WHISPER_BIN="$WHISPER_BIN"
export LECTURE_WHISPER_MODEL_DIR="$WHISPER_MODEL_DIR"
exec "$LECTURE_BIN" "\$@"
EOF
chmod +x "$LOCAL_BIN/lecture"

for shell_rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
  if [ -f "$shell_rc" ] && ! grep -q 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"' "$shell_rc"; then
    printf '\n# LectureCLI\nexport PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"\n' >> "$shell_rc"
  fi
done

echo
echo "Installed."
echo "Open a new terminal or run:"
echo '  export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
echo
echo "Then use:"
echo "  lecture devices"
echo "  lecture start 3 clase-fisica2"
