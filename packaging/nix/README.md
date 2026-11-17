# Nix package

The flake exposes `didcord` (also the default package) and a development shell for
`x86_64-linux` and `aarch64-darwin`. Build on the matching host:

```sh
nix build .#didcord
nix run .#didcord
nix develop        # cargo, rustc and the native build inputs
```

Linux installs `result/bin/didcord`, a `cz.viceverse.didcord.desktop` entry and icons.
macOS installs `result/Applications/Didcord.app` with `result/bin/didcord` linking to
the bundle executable. Both include the notices, font/asset licenses and the
corresponding MPL-2.0 hpke-rs source under `result/share/doc/didcord` (inside the
bundle `Resources` on macOS).

The package version is the Cargo workspace version, as with `cargo build`. Nix
installations are updated through Nix; the in-app updater does not replace store
paths. The macOS bundle is not Developer ID signed or notarized.

The build needs `git` and `rustPlatform.bindgenHook`: the bundled BoringSSL is patched with
`git apply` and its bindings are generated through libclang.

CI builds both systems and compares installed notices and source with the checkout.
A successful build does not establish live Discord, keyring, screen capture or
physical audio compatibility. Linux still needs a graphical session, GPU driver,
Secret Service provider and an XDG desktop portal backend.
