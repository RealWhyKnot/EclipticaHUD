# EclipticaHUD

Boss target and damage HUD for the VRChat world Ecliptica. It tails the VRChat output log and shows the current boss and its target, your DPS and damage taken, run history, and an event log, as an always-on-top desktop panel and as a SteamVR overlay on the left controller.

Everything comes from the log VRChat already writes: no mods, no OSC, nothing injected. The log only records your own damage, so other players' DPS is not possible.

## Layout

The log flows one way: `vrchat` reads it, `game` turns lines into state, `hud` draws that state.

- `src/vrchat/log.rs` tails the VRChat output log and follows rotation
- `src/game/event.rs` turns log lines into events
- `src/game/state/` folds events into runs, fights and live stats; `apply.rs` has one method per event, `tests/` one file per topic
- `src/game/run.rs` the run, fight and stage records; `source.rs` names attackers; `names.rs` display names
- `src/hud/timeline.rs` merges hits, aggro switches and deaths into the event log rows
- `src/hud/render/` draws both windows into GDI bitmaps: `main_panel.rs` one method per card, `log_panel.rs`, `hit.rs` hit testing and tooltips, `layout.rs` every rectangle, `theme.rs` colors and fonts
- `src/hud/app/` the headless HUD: `tick.rs` the frame loop, `nav.rs` run and phase paging, `logwin.rs` the log window, `settings.rs`, `anim.rs`
- `src/hud/win32/` windows, message loops and the settings files under `%APPDATA%\EclipticaHUD`
- `src/discord/` optional Rich Presence: `ipc.rs` the pipe protocol, `activity.rs` what gets shown. Off by default; the DISCORD dot in the footer toggles it
- `src/vr.rs` OpenVR overlay, `src/update.rs` self-update, `src/process.rs` process checks, `src/scan.rs` the `--scan` command

Nothing under `src/game` or `src/vrchat` touches a window, so all of it runs under `cargo test`.

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
