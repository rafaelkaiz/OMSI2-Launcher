# OMSI 2 Launcher

**A fast, modern launcher for the vanilla OMSI 2 bus simulator.** Pick the map, driver, date and time, press *Start* and OMSI 2 opens and loads the map for you, with no clicking through the start screen.

> **Version 0.9** (see the [release notes](https://github.com/rafaelkaiz/OMSI2-Launcher/releases)). [Español](README.es.md) · [Web page](https://rafaelkaiz.github.io/OMSI2-Launcher/) · [Download](https://github.com/rafaelkaiz/OMSI2-Launcher/releases)

![The launcher](docs/img/launcher.png)

## What it does

- **One-click start**: open OMSI 2 Launcher, pick your map and driver, and it starts OMSI 2 and loads the map: last map state, map without buses, or a saved situation.
- **Date and time**: like OMSI's start screen: change them in a window of their own (time, the four times of day, date, weekday and the map's holidays) or keep the situation's.
- **Choose where to appear**: with "Load map without buses", pick the map's start place in the launcher and it answers OMSI's "Set position" window for you.
- **Tutorials 1–4** straight from the launcher.
- **OMSI's options window, rebuilt**: General, Advanced, Graphics, Graphics (advanced), Sound and AI traffic, laid out like the original. Load option presets and save your current options as a new preset.
- **Drivers**: edit, create and delete drivers, and see the staff file (stops, distance, comfort rating, buses driven).
- **Looks like OMSI**: the original orange design, plus a dark and a light theme. Opens full screen with a loading screen that stays up until OMSI is loading the map (F11 toggles full screen).
- **Languages**: Español, English, Deutsch, Polski, Čeština, Français and Português, plus any language in OMSI's `Languages` folder (add a `<CODE>.txt` file for the launcher's own texts; a template is created for you). The first run picks your Windows language.
- **Removes the `closecheck` file** OMSI leaves behind after a crash (optional).
- **Checks GitHub for new versions** (it only tells you; it never downloads or replaces anything by itself).

![Options](docs/img/options.png)

![Launcher settings](docs/img/settings.png)

![Personnel file](docs/img/staff.png)

![Tutorials](docs/img/tutorials.png)

## It never touches your OMSI files

The launcher was built around one rule: **the original OMSI 2 files are not modified, with one optional exception you choose yourself.** The only things it writes are:

| What | Where | Safety |
| --- | --- | --- |
| `options.cfg` | OMSI folder | only when you press *Accept* in Options; a backup is made first |
| `Drivers\*.odr` | OMSI folder | only when you edit, create or delete a driver; a backup is made first |
| New preset files | `option_presets\` | new files only, existing presets are never overwritten |
| Two plugin files | `plugins\` | `OMSI2Launcher.opl` and `OMSI2LauncherPlugin.dll`, only when you press *Install plugin* |
| `Omsi.exe` | OMSI folder | only the optional *4 GB patch* (one header bit), when you press its button; a backup is made first and it can be removed |
| Its own settings, log and backups | `%APPDATA%\OMSI2-Launcher\` | the 3 newest backups of each file are kept |

Nothing is written while OMSI 2 is running.

## The plugin

To press *Start* (and the other start-screen buttons) without simulating mouse clicks, the launcher installs a tiny plugin into OMSI's `plugins\` folder. The plugin only listens for commands from the launcher and calls OMSI's own button handlers. **In this version the plugin is required to start OMSI from the launcher**; install it from *Launcher settings → Inicio directo (plugin)*, and remove it from the same place whenever you like.

The plugin currently recognises **OMSI 2 version 2.3.004** only.

## Install

1. Download `OMSI2-Launcher-0.9-win64.zip` from the [releases page](https://github.com/rafaelkaiz/OMSI2-Launcher/releases) and unzip it anywhere.
2. Run `OMSI2-Launcher.exe`. It finds OMSI 2 by itself (Steam or Aerosoft default folders); if it cannot, it asks where it is.
3. Open **Launcher settings** and press **Install plugin**.
4. Choose your map and driver and press **Start**.

Windows 10/11, 64-bit. The program is not code-signed, so Windows SmartScreen or your antivirus may warn about it the first time: it is a new, unknown file, not a detected threat. You can review and build the source yourself.

## Known limitations

- The *Keyboard* and *Controller* tabs of the options window are not implemented yet (use OMSI's own options for those).
- The plugin only recognises OMSI 2 2.3.004.
- Polish and Czech translations have not been reviewed by native speakers yet.
- The tutorial icons are the launcher's own drawings, not OMSI's.

Found a bug? Please [open an issue](https://github.com/rafaelkaiz/OMSI2-Launcher/issues) and attach `%APPDATA%\OMSI2-Launcher\launcher.log` (and `plugin\plugin.log` next to it).

## Build from source

You need [Rust](https://rustup.rs/) (edition 2024) on Windows.

```bash
# the launcher
cargo build --release

# the plugin (32-bit, because OMSI 2 is a 32-bit program); its DLL is embedded into the launcher
rustup target add i686-pc-windows-msvc
cd plugin
cargo build --release --target i686-pc-windows-msvc
copy target\i686-pc-windows-msvc\release\OMSI2LauncherPlugin.dll dist\OMSI2LauncherPlugin.dll
cd ..
cargo build --release
```

Run the tests with `cargo test --release -- --test-threads=1`. What each key of OMSI's `options.cfg` means, and what a preset does with it, is in [docs/options.md](docs/options.md). The `tools\` folder holds the PowerShell and disassembler helpers used to study OMSI's start screen.

## Support the project

If the launcher saves you time and you want to say thanks, you can [buy me a coffee](https://buymeacoffee.com/6zEaCpzmYU). It is entirely optional.

## License

OMSI 2 Launcher is free software, released under the [GNU General Public License v3.0 or later](LICENSE). Copyright (C) 2026 Rafael Casaucao. You may use, study, share and modify it; if you distribute a modified version you must do so under the same license and make its source code available.

## Credits and legal

OMSI 2 is a product of MR-Software and Aerosoft. This is an unofficial, independent community project, not affiliated with or endorsed by them, and it contains no OMSI 2 files. Support and add-ons for OMSI live at the [OMSI forum](https://forum.omnibussimulator.de/forum/).
