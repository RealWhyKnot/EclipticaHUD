# EclipticaHUD

A small overlay for the VRChat world Ecliptica. It reads the VRChat output log while you play and shows who the current boss is targeting, your DPS, and what just hit you - as an always-on-top desktop panel and, when SteamVR is running, as an overlay attached to your left controller.

Everything comes from the log VRChat already writes, so there are no mods, no OSC setup, and nothing injected into the game. I made this because boss aggro in Ecliptica switches silently and I wanted to see at a glance who needs to run.

## What it shows

- Current boss and the player it is targeting, with a history of recent switches
- A short blip whenever the boss switches targets, so you hear the swap without looking
- Your DPS over the last 10 seconds, DPS for the whole fight, and total fight damage
- Personal damage summary from your last boss kill (strike + non-strike)
- Recent damage you took, with the enemy and attack that caused it
- Current stage, run progress, and class
- Every past run and boss fight, with damage, DPS, duration, and kill totals

The log only records your own damage, so DPS for other players isn't possible. Target names are whoever the boss aggros, which the world exposes for every player.

## Usage

Grab a release or build it, then run `ecliptica-hud.exe`. The panel appears bottom-right; drag it anywhere, close it with the x or Escape. Position is remembered.

Click the target name to mute or unmute the target-change sound. A small muted icon shows on the boss card while it's off, and the choice is remembered.

The arrows next to the run row and on the boss card step back through earlier runs and their fights; stepping forward past the newest returns to the live view. A run ends when you're back in the lobby, leave the world, or restart VRChat. On startup the HUD reads whatever logs VRChat still has on disk, so the last few days of runs are there without keeping it open.

If SteamVR is running (or starts later), a wrist overlay appears on the left controller automatically.

The footer shows the running version. When a newer release is out it shows the new tag instead; click it and the app downloads the release, verifies its SHA256, swaps itself, and restarts.

`ecliptica-hud.exe --scan [logfile]` parses a log and prints every recognized event plus totals to stdout, which is handy for checking what the parser sees. Without an argument it scans the newest VRChat log.

## Building

Requires the MSVC toolchain, CMake, and LLVM (libclang) for the OpenVR bindings.

```
cargo build --release
```

Commit subjects are conventional commits; release notes are generated from them. Enable the local check with:

```
git config core.hooksPath .githooks
```

Releases are tagged `vYYYY.M.D.N` (CalVer), with `-beta` prereleases tagged nightly when main has moved.

## License

MIT
