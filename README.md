# veltrio-core

The shared Rust core behind every Veltrio app: Stremio addon protocol,
IPTV (M3U + Xtream Codes), the torrent engine (librqbit) and debrid
clients, exposed to Swift and Kotlin through [UniFFI](https://mozilla.github.io/uniffi-rs/).

## Develop

```bash
cargo test -p veltrio-core          # unit tests
cargo test -- --ignored             # live network tests (opt-in)
```

## Artifacts

Each platform repository pins a release of this core in its `core.version`
file and downloads the matching artifact. Pushing a tag `vX.Y.Z` runs
`.github/workflows/release.yml`, which publishes:

| File | For | Contents |
| --- | --- | --- |
| `veltrio-core-macos.zip` | veltrio-macos | universal dylib + Swift bindings |
| `veltrio-core-apple-mobile.zip` | veltrio-ios | `veltrioFFI.xcframework` (device + simulator) + Swift bindings |
| `veltrio-core-apple-mobile-tvos.zip` | veltrio-tvos | the same, with tvOS slices (nightly Rust; may be missing) |
| `veltrio-core-android.zip` | veltrio-android, veltrio-androidtv | `.so` per ABI + Kotlin bindings |
| `veltrio-core-windows-x64.zip` | veltrio-windows | DLL + import lib + C# bindings (uniffi-bindgen-cs) |

Build any of them locally with `scripts/package-*.sh`; they land in `dist/`.
For an app you can also point its `scripts/fetch-core.sh` at a local
checkout instead of a release: `VELTRIO_CORE_DIR=../veltrio-core`.

To release: bump `version` in `Cargo.toml`, commit, then
`git tag vX.Y.Z && git push origin vX.Y.Z`.

## Patched dependencies

See `patches/README.md`.

## License

Dual-licensed under either of Apache-2.0 or MIT.
