# Rebrand validation — November 18, 2026

Offline validation on macOS 27.2 arm64, Apple M1 (7-core GPU), 8 GiB RAM,
2560×1600 Retina display at 2× scale, Metal, pinned Rust 1.99.1 and locked
dependencies. No account, Discord transport, microphone or camera was used.

## Changes and checks

- Full `cargo xtask check` with the documented Command Line Tools `LIBRARY_PATH`
  workaround: formatting, Clippy, workspace tests, production build and policy.
- Extension workspace tests, Clippy and release Wasm builds; committed/rebuilt
  native host ABI checks and immutable legacy App Toolbox check.
- Catalog validation of 18 packages, six license-policy fixtures, login handoff
  and xtask-workspace harnesses.
- Both 30-second fuzz targets, using cargo-fuzz 0.13.2 and nightly-2026-09-09.
  Canonical lockfile ordering is retained; the second run exited successfully
  without lockfile drift.
- `close_settings_preserves_later_interactive_input` failed before the one-shot
  Escape fix and passed afterwards. CI runs the example's tests explicitly.
- Standard voice-enabled `cargo xtask package`, `codesign --verify --strict` and
  `plutil -lint` passed. The bundle is ad-hoc signed, not notarized.

The first package attempt exposed the missing Icon Composer source and unavailable
`actool`. Packaging now stages the existing ICNS, without generating appearance
variants. Native release builds required the documented Swift library path with
standalone Command Line Tools. Existing third-party Wry warnings remain visible.

## Native interface

The release `profile_preview` example was built with `--features demo`. It contains
synthetic state and no service adapters. Native framebuffer captures were opened
and inspected at 1120×760 dark/light and 680×560 dark logical pixels. The narrow
fixture keeps its composer visible and wraps message bodies; its member list is
hidden automatically. `Loading people…` is fixture state, not a service request.

```sh
target/release/examples/profile_preview --demo --page=chat --close-settings --output=chat.png --width=1120 --height=760
target/release/examples/profile_preview --demo --page=chat --close-settings --output=narrow.png --width=680 --height=560
target/release/examples/profile_preview --demo --page=chat --close-settings --output=light.png --width=1120 --height=760 --light
```

Interactive checks used the same executable in a temporary, separately identified
app bundle so the native UI tool could select it. ASCII typing, Thai text set
through native accessibility, and Left/Backspace editing were observed. The Thai
sample `สวัสดี ทดสอบภาษาไทย น้ำ` rendered with visible vowels and tone marks.
Direct Thai `typeText` and paste did not reliably insert Unicode through this
automation tool; physical Thai keyboard/IME composition remains unverified.

Native drag attempts returned `noWindowsAvailable`. Two explicit launch sizes
were checked instead; physical drag resizing and saved geometry were not verified.
Quit/relaunch and three sequential capture launches succeeded. The offline fixture
does not persist drafts or window geometry. The standard packaged executable
correctly refuses `--demo` before authenticated startup; the captures are not from
that production package.

## Package comparison

Production executable sources are `9da2113` plus test/format/lockfile/metadata
repairs for the baseline, and `27bd690` for the byte-accounting fix. The baseline
release executable was saved after its successful compilation, before the failed
icon-staging step. Its SHA-256 before bundle re-signing was
`f4a96c86f9e81f01ad34d2ba13fc0ab05338606bbd6bd1915281e1463cd3eaff`.

The baseline was restaged with exactly the final package's resources and its saved
executable, then ad-hoc signed. Both ZIPs used `ditto -c -k --keepParent` with the
same `dist` root name. These are like-for-like staged payload measurements, not a
claim that the original packaging command succeeded before the icon fix.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Signed executable | 77,088,016 B | 77,088,016 B | 0 B |
| Installed staging payload, 239 files | 82,781,747 B | 82,781,747 B | 0 B |
| ZIP | 52,098,869 B | 52,098,869 B | 0 B |

Package resources include documentation at measurement time. Subsequent evidence
documentation adds bytes to the delivered package and is not a runtime change.

## Idle and capture baseline

After all local builds/fuzz runs finished, run the release fixture as:

```sh
target/release/examples/profile_preview --demo --page=chat --close-settings --interactive --width=1120 --height=760
```

Three new processes, 8-second warmup each, then 21 readings over approximately
20 seconds at one-second intervals using `ps -p PID -o time=,rss=`. CPU is the
process CPU-time delta divided by measured elapsed time, expressed as a percentage
of one core. No typing, pointer injection or build ran during sampling. RSS covers
the main process; helper-process memory and GPU allocations were not sampled.

| Run | CPU, one core | Peak RSS | Settled RSS |
| --- | ---: | ---: | ---: |
| 1 | 0.04925% | 156,816 KiB | 156,768 KiB |
| 2 | 0.04927% | 156,944 KiB | 156,896 KiB |
| 3 | 0.04922% | 157,088 KiB | 149,328 KiB |

Median CPU is 0.04925%; median settled RSS is 156,768 KiB (153.09 MiB).
Short samples and CPU-time quantization limit precision. This is a synthetic idle
baseline, with diagnostics off, not an application-wide performance improvement.

Three sequential launches reached a saved native framebuffer in 2.679, 1.271 and
1.269 seconds (10 ms file-existence polling). This includes the capture fixture's
minimum one-second wait, screenshot encoding and file publication. It is not
time-to-first-present, a cold-start measurement or a startup p95.

Raw readings: [idle](idle.json), [capture launches](startup.json),
[package sizes](package.json). Live Discord, physical media, screen readers,
Windows/Linux desktop behavior, signed installers and notarization remain unverified.
