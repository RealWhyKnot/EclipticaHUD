# Privacy

EclipticaHUD reads your VRChat log on your PC. Nothing leaves your PC unless you turn on one of the features below. All of them send data to `ecliptica.whyknot.dev`.

## Shared damage

Turn it on from the party window (the button next to settings). It's off by default.

During a boss fight the HUD sends your damage dealt and taken at most once a second, along with hashes of the instance and your VRChat user ID. Names come from each player's own log, and the server keeps nothing on disk.

It also sends the stages and bosses of your current run and when each one started and ended. If you join partway through a run, the HUD fills in the stages and bosses you missed from players who share, and the run line says "syncing with party" until it has them. That history only shows for the current run and isn't saved.

## Share fight data

The HUD asks once whether you want to share fight data, and it stays off unless you say yes. You can change your answer later with the Share fight data switch in settings.

With it on, the HUD sends the Ecliptica lines it reads from your log about every ten minutes, to help improve the HUD. That includes:

- boss and enemy names, stages, the damage you dealt and took and what hit you
- the player count, the world ID, the instance number and the HUD version
- the run, fight and stage totals the HUD works out from those lines
- saved shared fights, as numbers with no names attached

In other worlds it only notes that you left Ecliptica. Nothing from before you turned the switch on is sent. The server keeps the uploads as files.

### What is removed before upload

- Player names and user IDs are taken out on your PC. Other players show up as P1, P2 and so on, and you show up as "you". The numbering starts over for each log file and for each saved fight.
- The instance keeps its number but loses the owner and region tags. A named instance is sent as a hash instead of its name.
- Each upload starts with the date, the clock time of its first line and your UTC offset. After that, each line only keeps how long after the previous one it came.
- Each log file gets an ID made from its name and a random key that stays on your PC, so the server can join uploads from the same session.

## Leaderboard

Turn on Leaderboard in settings to put your boss kill times on [ecliptica.whyknot.dev](https://ecliptica.whyknot.dev) under your VRChat display name. It's off by default. While it's on, the HUD sends your fight data and VRChat name.

New kills show as pending while they're checked, and only kills made with Leaderboard on count. Each entry has the kill time, how many players were in the instance, your class, the stage, damage dealt and taken, hits taken, the biggest hit you took, deaths and graphs of damage dealt and taken over the fight.

The first time, the HUD makes a random key in `%APPDATA%\EclipticaHUD\board-key.txt`. That key owns your name on the board, so another PC can't post under it.

To stop, turn Leaderboard off. That stops new kills from being sent. To remove kills already on the board, click Delete next to BOARD KILLS in settings.

## Stored on your PC

The HUD copies the Ecliptica lines it uses from your VRChat log into `%APPDATA%\EclipticaHUD\history.db`, so your runs stay in the panel after VRChat deletes its old logs. Shared fights get saved there when they end. The file stays on your PC.
