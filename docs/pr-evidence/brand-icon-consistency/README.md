# Brand icon consistency

Baseline: `d812cfe`, on the existing sign-in/Thai locale PR branch. The working tree was clean.
Verification uses offline synthetic data only; no Discord account, call or microphone was used.

## Audit

| Surface | Before | Change |
| --- | --- | --- |
| Home / Direct Messages tile | Old chat-wave in atlas cell 58 | Flat version of the current mascot |
| Updates card | Same old shared atlas cell | Updated through the same asset |
| Login header | Same old shared atlas cell | Updated through the same asset |
| macOS and Linux tray pixmap | Full-colour plate; alpha became a solid tile when tinted | Black mascot on transparency |
| Linux symbolic tray | Embedded full-colour PNG, unable to follow `currentColor` | Same vector path as the flat mascot, with the tray stylesheet |
| Sign-in card, button and loading view | Current full-colour mascot | Already current |
| Windows, macOS and Linux packaged app icons; README | Current full-colour mascot | Already current |

The mark source had already been replaced by an embedded full-colour PNG, but the atlas still
held the chat-wave and the generator still pinned the previous vector's hash. Regeneration
could not pass its source-hash check. The replacement flat vector is traced from the bundled
`assets/brand/didcord-1024.png`, retaining eye highlights and transparent eyes/mouth without
the background plate. The generator now pins that vector.

One-off pixel checks confirm only atlas cell 58 changed: all other cells, including unused
padding, decode identically to the baseline. Source-hash validation, monochrome tray pixels,
transparent plate, and matching packaged symbolic path/view box were checked separately.
No pixel-layout unit test was added, per the repository's test policy.

## Native screenshots

The before/after pair uses the existing native framebuffer capture example on macOS, Metal,
1120×760 logical points at Retina 2×, dark appearance, and the same offline chat fixture:

```sh
LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift/macosx \
  cargo build --release --locked -p didcord --features demo --example profile_preview
target/release/examples/profile_preview --demo --page=chat --close-settings \
  --width=1120 --height=760 --output=/tmp/didcord-icon-before.png
```

After rebuilding with the changed assets, use the same capture command with
`--output=/tmp/didcord-icon-after.png`. The skeleton member list belongs to this existing
fixture; it is not a live Discord response. Historical screenshots elsewhere are unchanged.

## Checks

- Regenerated the full atlas with `python3 tools/generate-icons.py --resvg /tmp/resvg-tool/bin/resvg`;
  all pinned upstream and local source hashes passed. The PNG was then losslessly indexed;
  decoded RGBA pixels were compared before and after compression.
- `LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift/macosx cargo xtask check`
  passed, including format, Clippy, workspace tests, the no-default-features build and policy.
  The first sandboxed attempt could not bind localhost in 11 fixture tests; the same command
  passed outside the sandbox. Vendored `wry` warnings are unchanged.
- Windows/Linux native rendering and desktop tray-host recoloring were not exercised on this
  macOS machine. No authentication code changed in this follow-up.

## Package comparison

Baseline and changed builds both passed `cargo xtask package` using locked Rust 1.99.1,
normal voice-enabled release settings and local ad-hoc signing. Before rebuilding, the
baseline app was copied into `/tmp/didcord-icon-baseline` to preserve it. Each staging
payload was separately ZIP-compressed with Python `zipfile`, `ZIP_DEFLATED`, level 9,
including all files in `dist` except existing `.zip` archives; this avoids reusing an old ZIP.

| Metric / method | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Packaged executable (bytes) | 77,186,528 | 77,186,528 | +0 (+0.000%) |
| Installed .app payload (bytes) | 82,824,295 | 82,824,295 | +0 (+0.000%) |
| Compressed staging payload (ZIP level 9, bytes) | 52,025,102 | 52,018,921 | -6,181 (-0.012%) |
| Atlas PNG (bytes) | 27,326 | 30,467 | +3,141 |
| Tray PNG (bytes) | 8,579 | 1,712 | -6,867 |

These are artifact-byte comparisons, not runtime performance measurements. CPU, process RSS
and frame/startup latency were not sampled for this asset-only follow-up.
