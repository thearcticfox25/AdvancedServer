## About Advanced Server

Advanced Server is an implementation of a [Sonic.EXE: TD2DR](https://gamejolt.com/games/disaster2dr/780541) game-compatible server. It behaves nearly the same way, supports the original command set + many new features, custom improvements, and comes with flexible configuration and an integrated terminal for server management.

**License:** AGPL v3

## Features

- **Full TD2DR compatibility** — works seamlessly with original game clients
- **Built-in Terminal with administrative tools** — the simple command-line interface with basic commands for management, moderation, and player administration.
- **Flexible Configuration** — customizable server behavior and gameplay parameters in Config.toml.

## Commands

Commands are prefixed based on their purpose and origin:
- `.` — original TD2DR player commands
- `:` — new player commands added in AdvancedServer
- `?` — administrative commands (system administrator only)

Use `.help` in lobby, or `?help` in the terminal for a list of available commands.

## Credits

- This project has referenced game protocol realization from [BetterServer-OSS](https://github.com/teamexeempire/betterserver-oss) (MIT license) by Team EXE Empire.
- Overhaul game type's origin is from [Gamemode Cycle](https://gamejolt.com/games/TD2DRgmcycle/919987) server mod, that was created by IcedCoffee & ColdestTea.
- The disable_timer function is a reimplementation of Banana8870's [TD2DR No Timer](https://gamejolt.com/games/no_timer_td2dr/923609) Server Mod.
- Networking is built on [ENet](https://github.com/lsalzman/enet) (MIT license) by Lee Salzman, via its Rust port [rusty_enet](https://github.com/jabuwu/rusty_enet) (MIT license) by jabuwu.
- The per-tick event/packet flood cap is adapted from vinny's disasterserver, used here with permission.

**Note 1:** AdvancedServer is a server-only implementation, not a full-stack client+server mod. Instead, it's designed to extend the experience of the **original** game version.
To use AdvancedServer: launch the original Sonic.EXE: TD2DR game and connect to the server like you would to any custom player server. Do not look for a custom client build — AdvancedServer works with the original game client only.
**Note 2:** Playing on third-party or modified clients is not supported and not guaranteed to work.

---

**!!! NOTICE:** This version is unstable. Use at your own risk in production environments.
