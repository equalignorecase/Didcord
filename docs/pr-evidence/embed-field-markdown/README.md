# Embed field Markdown

Baseline production renderer: `94fcf51` (`origin/main`). Work uses the separate
`fix/markdown-bold` worktree; the existing branding/Thai PR branch is unchanged.
All data and usernames in the regression test and screenshots are synthetic.

## Reproduction and fix

An embed field name such as `#1 **synthetic#99999**` used a raw `RichText` label,
while the same text in descriptions and field values already used the bounded Markdown
renderer. The raw label displayed asterisks and did not select the bundled heavier face.

Field names now reuse the existing Markdown renderer with a scoped semibold body face.
The field value keeps its normal body face; inline code retains literal asterisks.
Names have their own widget ID namespace and use the upper half of each embed's existing
64 cache slots. Up to 25 field values use the lower half, so names and values do not
collide. The existing 512-entry / 1 MiB cache limits are unchanged.

The single regression test exercises the actual rendered embed, checking the name's
visible text and heavier font, literal inline code, and an unchanged normal-weight value:

```sh
LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift/macosx \
  cargo test --locked -p ui --lib ranked_embed_field_names_render_bold_markup_instead_of_asterisks
```

Before the fix it failed with `#1 **synthetic#99999** / ` followed by the literal
backtick-delimited code. After the fix it passed. No parser replacement or dependency
was introduced.

## Native capture

The new `profile_preview --page=embed-markdown` fixture puts the same rank/name pattern
in chat, an embed description and an embed field name. It also includes italic text and
literal code in the value. This fixture was added before capturing the unchanged renderer;
the before image is not an older fixture disguised as the same scenario.

```sh
LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift/macosx \
  cargo build --release --locked -p didcord --features demo --example profile_preview
target/release/examples/profile_preview --demo --page=embed-markdown \
  --width=1120 --height=760 --output=/tmp/didcord-bold-before.png
```

After rebuilding with the fix, repeat with `--output=/tmp/didcord-bold-after.png`.
The pair uses macOS, Metal, a 1120×760-point window at Retina 2×, and dark appearance.
The member skeletons belong to the existing offline fixture, not live Discord responses.

## Measurement method

Normal voice-enabled, no-default-features packages are built with `cargo xtask package`.
The baseline `.app` is copied aside before rebuilding. Installed bytes sum regular files
in the `.app`; the staging ZIP includes files in `dist` except pre-existing ZIP archives,
using Python `zipfile.ZIP_DEFLATED`, level 9. Each revision gets its own archive.

Native UI samples run the same release example with `--demo --page=embed-markdown
--interactive --width=1120 --height=760`: eight seconds of warmup, then 21 readings at
roughly 0.5-second intervals. CPU is the change in `ps -o time` over measured wall time,
as percent of one core; RSS is `ps -o rss`, in KiB. The CPU counter has 0.01-second
resolution. Reported peaks cover the sampling window, not lifetime peak memory.
These are single idle-fixture runs, not an interaction-latency benchmark or a speed claim.
The example hosts the UI without a live transport; helpers/GPU memory and the full
application's background-work CPU are not measured.

## Verification

- `cargo xtask check` passed: format, Clippy, 1,575 passing tests (14 ignored),
  no-default-features build and policy checks.
- `cargo clippy --locked -p didcord --features demo --example profile_preview -- -D warnings`
  passed for the new fixture.
- Baseline and changed `cargo xtask package` builds passed, including local ad-hoc signing
  and signature verification. The existing vendored `wry` warnings remain.
- Rust 1.99.1 and locked dependencies were used. macOS build/test commands set
  `LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift/macosx`.
- No authentication code changed. Windows/Linux native rendering and live Discord behavior
  were not tested.

## Results

The dark before/after screenshots were inspected. The fixed field has no bold delimiters,
uses the heavier face, and keeps the value and literal code intact. The changed fixture was
also inspected in light appearance at 760×520 points; the card and text fit the viewport.

Environment: macOS 27.2.1, Apple M1, 8 GiB RAM, Metal, Retina 2×.

| Metric / method | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Packaged executable (bytes) | 77,088,016 | 77,088,016 | +0 (+0.000%) |
| Installed .app (bytes) | 82,725,783 | 82,725,783 | +0 (+0.000%) |
| Staging ZIP level 9 (bytes) | 51,944,993 | 51,945,458 | +465 (+0.001%) |
| Idle fixture CPU (% of one core) | 1.0695 | 1.0698 | +0.0003 percentage points |
| Median sampled fixture RSS (KiB) | 146,480 | 146,384 | -96 (-0.066%) |
| Peak sampled fixture RSS (KiB) | 146,480 | 146,384 | -96 (-0.066%) |

CPU was essentially identical in these short runs; the RSS difference is 96 KiB.
Neither difference establishes a performance improvement. Raw UI samples are in
`before-runtime.json` and `after-runtime.json`; package numbers count artifact bytes.
