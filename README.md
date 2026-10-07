# EclipticaHUD

Boss target and damage HUD for the VRChat world Ecliptica. It reads the VRChat output log and shows the current boss and who it's targeting, your DPS and damage taken, a run history and an event log. You get it as an always-on-top desktop panel and as a SteamVR overlay on your left controller.

No mods or OSC. The log only records your own damage, and that's why other players' DPS isn't shown anywhere in the HUD.

## Building

You need the MSVC toolchain, CMake and LLVM (libclang, for the OpenVR bindings).

```
cargo build --release
```

Commit subjects follow Conventional Commits and the release notes are built from them. To turn on the local commit check:

```
git config core.hooksPath .githooks
```

Releases are tagged `vYYYY.M.D.N`. A `-beta` prerelease is tagged nightly when main has new commits.

## License

MIT
