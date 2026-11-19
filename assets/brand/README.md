# Didcord brand assets

Didcord's own application mark: a smiling pale mascot on a dark blue rounded plate.
These files are Didcord design assets, not Apple, Discord or any
other vendor's branding, and no third-party font is embedded in them.

- `didcord-mark.svg` — white vector mascot traced from `didcord-1024.png`, with transparent
  eyes and mouth and no plate. Its square view box centers the mascot for small glyphs.
  `tools/generate-icons.py` rasterizes this one into the shared icon atlas as `didcord-mark`,
  which the interface tints at draw time.
- `didcord-1024.png` — 1024×1024 sRGB RGBA full-colour artwork.
- `didcord-tray.png` — 72×72 black-on-transparent render of `didcord-mark.svg`, embedded by
  `crates/platform` as the macOS menu bar template image; macOS tints it per appearance, so only
  its alpha is used. Regenerate with:

  ```sh
  sed 's/fill="#fff"/fill="#000"/g' assets/brand/didcord-mark.svg |
    rsvg-convert -w 72 -h 72 -o assets/brand/didcord-tray.png
  ```

The packaged platform icons built from the same artwork live in `packaging/`
(`macos/Didcord.icns`, `windows/Didcord.ico`, `linux/hicolor/*`). macOS packaging
copies the bundled ICNS; Icon Composer sources and appearance variants are not included.

The Linux `didcord-symbolic.svg` is the same flat mascot with the `ColorScheme-Text` stylesheet
and `currentColor` fill used by the tray adapter. Keep it synchronized with `didcord-mark.svg`.
The full-colour platform icons already carry the mascot; do not replace them with the flat mark.

The interface accent (`design::DEFAULT_PRIMARY_RGB`, `#1a72e8`) is in the same blue family and
is tuned so white text on it keeps a 4.5:1 contrast ratio.
