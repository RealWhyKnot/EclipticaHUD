# EclipticaHUD

Boss target and damage HUD for the VRChat world Ecliptica. It tails the VRChat output log and shows the current boss and its target, your DPS and damage taken, run history, and an event log, as an always-on-top desktop panel and as a SteamVR overlay on the left controller.

Everything comes from the log VRChat already writes: no mods, no OSC, nothing injected. The log only records your own damage, so other players' DPS is not possible.

## Layout

- `src/parse.rs` turns log lines into events
- `src/state.rs` folds events into runs, fights and live stats
- `src/log.rs` merges hits, aggro switches and deaths into the event timeline
- `src/render.rs` draws both windows into GDI bitmaps and owns hit testing
- `src/app.rs` animation state and input handling
- `src/ui.rs` Win32 windows, timer and persistence under `%APPDATA%\EclipticaHUD`
- `src/vr.rs` OpenVR overlay, `src/update.rs` self-update
- `src/discord.rs` optional Discord Rich Presence over the local IPC pipe, with a Join button and invites. Off by default; the DISCORD dot in the footer toggles it

`ecliptica-hud.exe --scan [logfile]` parses a log and prints every recognized event plus totals, which is the quickest way to check what the parser sees.

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
