## About Advanced Server

Advanced Server started as a server for [Sonic.EXE: TD2DR](https://gamejolt.com/games/disaster2dr/780541). It is still a server, but now the game is built onto it: a Rust port of TD2DR that lives in the same program. The game is not the goal in itself. The point is that the server and the game share one set of resources and one simulation (the physics, the levels, the characters, the gameplay numbers). That shared base is the groundwork for an anticheat that is part of the foundation rather than a check bolted on from outside.

The server runs every round itself, on the same code the game uses to predict its own player. A client sends only its buttons, and the server decides what they do. Nothing a client says about positions, hits, rings or objects is taken as true, so there is nothing to lie about.

**License:** AGPL v3

## One program, two roles

```
AdvancedServer                     the server
AdvancedServer --play [options]    the game
AdvancedServer --help              every option
```

- **Server:** lobbies, voting, the rounds themselves, moderation, and an integrated terminal for administration. It is configured in `PersistentData/Server/Config.toml`.
- **Game:** the TD2DR client, ported from the original GameMaker project. It plays online on an Advanced Server or alone on any map.
- **Shared:** `Resources/` (textures, sounds, maps, fonts) and `PersistentData/Core/` (the gameplay numbers: physics, characters, levels). The server sends its numbers to every player at the start of a round, so everyone plays by the server's rules.

## Compatibility

The original game client cannot join: the protocol changed from "the client reports, the server checks" to "the client sends input, the server simulates". Play with this program's own game (`--play`).

## Commands

Commands are prefixed based on their purpose and origin:
- `.` - original TD2DR player commands
- `:` - new player commands added in Advanced Server
- `?` - administrative commands (system administrator only)

Use `.help` in the lobby, or `?help` in the terminal, for a list of available commands.

## Building

```
cargo build --release
```

A release build packs `Resources/` into `Resources.tar` next to the binary; a debug build copies the folder there. `PersistentData/` is written on the first start.

## Credits

Most of this project is Team EXE Empire's work. [Sonic.EXE: TD2DR](https://gamejolt.com/games/disaster2dr/780541) is theirs, and its GameMaker source is published under the MIT license (`LICENSE-ORIGINAL-EXEEMPIRE.txt`). From it come:
- the game itself: every object, script, room and shader of the original, ported to Rust (the client, and the simulation the server runs);
- everything in `Resources/`: the sprites, sounds, music and maps, converted from the original project;
- the network protocol, from their [BetterServer-OSS](https://github.com/teamexeempire/betterserver-oss) (MIT license).

On top of it, Advanced Server brings the architecture that conducts these scripts and resources (one program for the server and the game, the shared simulation, the server-run rounds and their protocol), and the new options and game modes not credited below.

The people behind the original game (its code, level design, sprites, music, voice acting and testing) are listed in its credits, which this game shows as they were. Part of the sprites and music are by third parties the original credits name (Sonic Team among them).

Also:
- The Overhaul game type comes from the [Gamemode Cycle](https://gamejolt.com/games/TD2DRgmcycle/919987) server mod, created by IcedCoffee & ColdestTea.
- The disable_timer function is a reimplementation of Banana8870's [TD2DR No Timer](https://gamejolt.com/games/no_timer_td2dr/923609) server mod.
- Networking is built on [ENet](https://github.com/lsalzman/enet) (MIT license) by Lee Salzman, via its Rust port [rusty_enet](https://github.com/jabuwu/rusty_enet) (MIT license) by jabuwu.
- The per-tick event/packet flood cap is adapted from vinny's disasterserver, used here with permission.

---

**!!! NOTICE:** This version is unstable. Use at your own risk in production environments.
