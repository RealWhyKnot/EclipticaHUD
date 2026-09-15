# EclipticaHUD

Boss target and damage HUD for the VRChat world Ecliptica. It tails the VRChat output log and shows the current boss and its target, your DPS and damage taken, run history, and an event log, as an always-on-top desktop panel and as a SteamVR overlay on the left controller.

Everything comes from the log VRChat already writes: no mods, no OSC, nothing injected. The log only records your own damage, so other players' DPS is not possible.

## Building

Requires the MSVC toolchain, CMake, and LLVM (libclang) for the OpenVR bindings.

```
cargo build --release
```

`cargo test` runs headless, including the pixel probes on the renderer.

Commit subjects are conventional commits; release notes are generated from them. Enable the local check with:

```
git config core.hooksPath .githooks
```

Releases are tagged `vYYYY.M.D.N` (CalVer), with `-beta` prereleases tagged nightly when main has moved.

## License

MIT
