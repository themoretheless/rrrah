#!/bin/bash
# Auto Cut — install/refresh the CEP panel for Premiere Pro (macOS).
#
#   ./install.sh              install or refresh
#   ./install.sh --uninstall  remove the panel
#
# The panel is symlinked, not copied, so editing this repo updates the
# installed panel immediately — reopen the panel in Premiere to reload.
set -euo pipefail

EXT_ID="com.autocut.premiere"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$SCRIPT_DIR/extension"
TARGET_PARENT="$HOME/Library/Application Support/Adobe/CEP/extensions"
TARGET="$TARGET_PARENT/$EXT_ID"

if [ "${1:-}" = "--uninstall" ]; then
	if [ -e "$TARGET" ] || [ -L "$TARGET" ]; then
		rm -rf "$TARGET"
		echo "Removed $TARGET"
	else
		echo "Nothing installed at $TARGET"
	fi
	echo "Restart Premiere Pro to drop the panel from the menu."
	exit 0
fi

if [ ! -f "$SRC/CSXS/manifest.xml" ]; then
	echo "error: $SRC/CSXS/manifest.xml is missing — run this from the repo root." >&2
	exit 1
fi

echo "==> Installing Auto Cut from: $SRC"
mkdir -p "$TARGET_PARENT"
rm -rf "$TARGET"
ln -s "$SRC" "$TARGET"
echo "    linked: $TARGET -> $SRC"

# Unsigned panels only load with PlayerDebugMode set.
# PPro 2024 and earlier use CEP 11; PPro 2025+ uses CEP 12.
for v in 11 12; do
	defaults write "com.adobe.CSXS.$v" PlayerDebugMode 1
done
echo "    PlayerDebugMode=1 set for CSXS 11 and 12"

# ffmpeg does the audio analysis; the panel finds it at these paths.
FFMPEG_BIN=""
for candidate in /opt/homebrew/bin/ffmpeg /usr/local/bin/ffmpeg /opt/local/bin/ffmpeg /usr/bin/ffmpeg; do
	if [ -x "$candidate" ]; then FFMPEG_BIN="$candidate"; break; fi
done
if [ -n "$FFMPEG_BIN" ]; then
	echo "    ffmpeg: $FFMPEG_BIN ($("$FFMPEG_BIN" -version 2>/dev/null | head -1 | cut -d' ' -f1-3))"
else
	echo
	echo "    WARNING: ffmpeg not found in the usual locations."
	echo "             Install it with:  brew install ffmpeg"
	echo "             or set the full path in the panel's Settings (gear icon)."
fi

PPRO_APP="$(ls -d /Applications/Adobe\ Premiere\ Pro\ * 2>/dev/null | sort -r | head -n 1 || true)"
if [ -n "$PPRO_APP" ]; then
	echo "    detected: $PPRO_APP"
else
	echo "    note: no Premiere Pro found under /Applications (panel is still installed)"
fi

cat <<'EOF'

Done. Next:
  1. Quit Premiere Pro completely, then reopen it.
  2. Open the panel:
       PPro 2025/2026:  Window > Extensions (Legacy) > Auto Cut
       PPro 2024 and earlier:  Window > Extensions > Auto Cut
  3. Open a sequence, click Analyze, review the plan, then Apply cuts.

Panel not in the menu? Clear the CEP cache and reopen Premiere:
  rm -rf ~/Library/Caches/CSXS/cep_cache/

Debugging: open http://localhost:8092 while the panel is running.
EOF
