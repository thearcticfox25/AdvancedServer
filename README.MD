# Intro
Here's the Advanced Server, a modded version of BetterServer for Sonic.EXE: TD2DR.
The server has more flexible configurations, functions, fixes and some IcedCoffee & ColdestTea's gamemodes.

# Nem Commands

:status – Shows server statistics.
:chance – Adjusts the player's execution probability.
:stop – Forces the current round to end.
:start – Forces a round to start immediately.

Admin Commands:
:unban – Removes a ban on a player.
:reboot – Restarts the server.
:vm – Allows voting for a specific map.

# Config Recommendations

Turn on apply_textchat_fixes to make chat working again as on v100.
Turn on charselect_mod_unlocked, if you're planning to host a huge server for over than 7 players.
Disable pallete_anticheat, if you're testing your palletes.
Disable g_config.results.enable or set its timer to 5 if results window annoys you.

# Getting UI to work (For those, who wants to compile their own build)
Place [SDL](https://github.com/libsdl-org/SDL/releases/tag/release-2.32.0) and [SDL_image](https://github.com/libsdl-org/SDL_image/releases/tag/release-2.8.5) DLLs from official [SDL github page](https://github.com/libsdl-org) into your server directory for UI to work

This isn't necessarily if you've added SDL DLLs into %PATH%
