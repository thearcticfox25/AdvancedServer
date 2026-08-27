# Advanced Server — Information Page

---

## About

This repository holds the source for the **Advanced Server Information Page** — the official website for [AdvancedServer](https://github.com/thearcticfox25/AdvancedServer), a Sonic.EXE: TD2DR-compatible multiplayer server mod. It lives in this repository's `website` branch.

The whole page is built to look and behave like an old-school desktop OS: draggable windows, a taskbar and start menu, a virtual filesystem, and a handful of built-in "apps" — all as static HTML/CSS/JS, no build step, no backend.

## Features

- **Fake desktop shell** — draggable/resizable windows, a taskbar, a start menu, and a virtual filesystem driving the desktop layout.
- **Built-in apps** — a toy Explorer, Browser, Video Player, and a Winver ("About this computer")-style dialog.
- **Mobile fallback** — a lightweight file-browser view is shown instead of the desktop shell on phones/tablets.
- **Live docs from GitHub** — the Introduction and Compiling & Running pages pull their content directly from AdvancedServer's README and Wiki at request time, so they never go stale.
- **Mirror warning** — flags visitors landing on unofficial mirrors of the site and points them back to the canonical URL.

## Credits

- Built around [marked](https://github.com/markedjs/marked) (MIT license) for rendering Markdown pulled from GitHub, vendored locally in `fakeroot.d/vendor/`.
- Built for [AdvancedServer](https://github.com/thearcticfox25/AdvancedServer).

---

**!!! NOTICE:** This site currently tracks the `ψ`-testing build (`v1.1.0.1.ψ`, see `config.js`) and may reference in-progress features or unpublished releases. Expect rough edges.
