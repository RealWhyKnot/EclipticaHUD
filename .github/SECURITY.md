# Security policy

EclipticaHUD reads your VRChat log and, only if you turn it on, sends data to ecliptica.whyknot.dev. What it sends is listed in [docs/privacy.md](../docs/privacy.md).

## Reporting

Please don't open a public issue. Use [a private security advisory](https://github.com/RealWhyKnot/EclipticaHUD/security/advisories/new) so we can work out a fix before anything is published.

I try to reply within 7 days. There's no bug bounty.

## In scope

- Data being sent that the settings say is off, or data that should have been stripped before upload (player names, user IDs).
- Posting to the leaderboard under someone else's name, or getting past the key in `board-key.txt`.
- The HUD updating itself from a file that doesn't match what the release published.
- Anything that lets another player or a web page run code on your PC through the HUD.

## Out of scope

- Anything that needs admin access to your PC already.
- Bugs in VRChat, SteamVR or the game itself.
