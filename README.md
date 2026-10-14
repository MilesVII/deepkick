> [!NOTE]
> The project is not reviewed and is provided as is

# DeepKick Lyrics
Puts lyrics into MP3 and FLAC metadata. Android app built with Tauri 2, a vanilla TS frontend and Rust, plus `deepkick`, a single-executable interactive console utility for the desktop that shares the same core.

Successor of *Seventh Lyrics Manager*, which stopped working once Android moved to the Storage Access Framework.

## What it does
- Pick files or whole folders through the system picker; supported files (`.mp3`, `.flac`) end up in a queue
- **Fetch lyrics**: a foreground service keeps the process alive while a Rust worker walks the queue, looks each track up on [LRCLIB](https://lrclib.net) (optionally falling back to lyrics.ovh) by artist / title / album / duration, and writes the result into the `USLT` frame (ID3v2) or the `LYRICS` Vorbis comment (FLAC)
- **Remove lyrics** from every file in the queue
- A notification is posted when the run is over; tapping it opens the **report** with the status of every file
- Tap any file to open the **editor**: view, edit, search online (pick between plain and synced candidates), save or remove its lyrics
- **Broken files are handled**: padding junk before the audio, malformed date frames (`1997 / 1998`), bogus header and frame flags left by old taggers (an extended-header flag without the header, data-length bits on frames that have none), and comment frames with no language code no longer fail the file. Cyrillic tags that old Windows taggers stored as CP1251 bytes labelled Latin-1 (shown as `Êîòÿòà` by players) are repaired on the fly, so the song is still found. Repairs are written back whenever the file is processed, lyrics or not, and the report says what was fixed. Reading never modifies a file: defective tags are patched in memory first. The repair can be switched off in the settings.

## Console utility (`deepkick`)
A single executable (`deepkick.exe` on Windows, no installer, no runtime DLLs to ship) with a full-screen terminal interface and a few commands for scripting.

```bash
deepkick                          # interactive interface
deepkick D:\Music "E:\More music" # interactive, with these folders already queued
deepkick fetch D:\Music           # no interface: fetch and print one line per file
deepkick remove -y D:\Music\Album
deepkick show song.flac           # tags and lyrics of one file
deepkick edit song.mp3            # edit the lyrics in your text editor, git-style
deepkick config editor "code --wait"
```
Dropping a folder onto `deepkick.exe` in Explorer opens the interface with that folder queued, and dropping one onto the running window adds it to the queue (the console types the path as keystrokes; they are recognised as a path when they form one that exists).

In the interface:

| key | action |
|---|---|
| `a` | browse for files and folders (Space adds, `a` adds the current folder, `g` goes to a typed path) |
| `p` | type, paste or drag paths onto the window (Tab completes) |
| `f` / `r` | fetch / remove lyrics for the queue, or only the filtered files |
| `Tab` | filter the queue by status; this is the report after a run |
| `e` / Enter | edit the selected file's lyrics in your editor |
| `s` | search LRCLIB and write a match (Enter) or edit it first (`e`) |
| `v` | view the lyrics |
| `x` / `X` | drop the selected file / clear the queue |
| `c` | settings |
| `Esc` | stop a run (the current file is always finished first) |

**Editing** works like git commit messages: the lyrics are written to a temporary file below a commented header and a scissors line, the editor is started and waited for, and whatever is below the scissors line when it closes is written back. An empty text removes the lyrics. The editor is taken from `DEEPKICK_EDITOR`, the `editor` setting, `VISUAL`, `EDITOR`, then `notepad` on Windows or `nano`/`vim`/`vi` elsewhere. GUI editors need their wait flag (`code --wait`, `subl -w`).

Settings live in `%APPDATA%\deepkick-lyrics\config.toml` (`~/.config/deepkick-lyrics/config.toml` on Linux; `DEEPKICK_CONFIG` overrides the path): `overwrite`, `prefer-synced`, `fallback`, `fix-encoding`, `editor`.

The executable carries the app icon (`crates/deepkick-cli/deepkick.ico`, embedded by its build script); rebuild the `.ico` from `src-tauri/icons/icon-source.png` if the logo changes.

Build it with:
```bash
cargo build --manifest-path src-tauri/Cargo.toml -p deepkick-cli --profile dist
# -> src-tauri/target/dist/deepkick.exe
```

## Layout
```
index.html            page shell (tabs: queue, report, editor, settings)
src/
  index.ts            defines custom elements and runs the units
  api.ts              typed invoke() wrappers + event subscriptions
  store.ts            non-reactive queue snapshot shared by units
  units/              one module per screen (queue, report, editor, settings, shell, navigation, toasts)
  components/         rampike custom elements (tabs, modal, popup, fieldset)
  css/                main / theme / utils / components
src-tauri/
  src/
    lib.rs            builder, plugins, command registration
    commands.rs       IPC commands
    state.rs          queue / run / report types
    worker.rs         the queue runner thread
    persist.rs        queue + report persisted in the app data dir
  crates/deepkick-core/   shared by the app and the console utility
    src/tags.rs       lofty-based lyrics read/write for MP3 and FLAC (tolerant parsing, year rescue)
    src/encoding.rs   repair of CP1251 / UTF-8 text stored as Latin-1
    src/lyrics.rs     LRCLIB + lyrics.ovh client (ureq, rustls)
    src/process.rs    statuses, run options, processing one file, run summaries
  crates/deepkick-cli/    the `deepkick` console utility
    src/main.rs       command line (clap)
    src/batch.rs      fetch / remove / show / edit / config commands
    src/tui/          interactive interface (ratatui): queue, browser, search, settings
    src/editor.rs     external editor round trip
    src/queue.rs      queue entries and the background runner
  plugins/deepkick/   local Tauri plugin
    src/mobile.rs     Rust side: calls into the Kotlin plugin
    src/desktop.rs    desktop stand-in (rfd dialogs, plain paths) for development on a PC
    android/          Kotlin: SAF pickers, fd hand-off, foreground service, notifications, insets, back handling
```

The file access model: Kotlin opens the `content://` document with `openFileDescriptor()` and hands the raw descriptor to Rust, which wraps it in a `File` and lets lofty rewrite the tags in place. No copies, no temporary files.

## Development
```bash
npm install
npm run tauri dev                 # desktop window (Windows/Linux/macOS), uses native dialogs
npm run tauri android dev         # on a connected device / emulator
npm run tauri android build --debug --target x86_64 --apk   # emulator APK
```

App icon: the Seventh Lyrics Manager glyph (`src-tauri/icons/icon-source.png`). The Android launcher icon is a hand-traced vector (`gen/android/app/src/main/res/drawable/ic_launcher_foreground.xml`) on a dark adaptive background, and the notification icon is a white silhouette of the same glyph in the plugin. `tauri icon` regenerates desktop icons fine, but it also overwrites the Android `mipmap-*` and `ic_launcher_background` resources, so restore those from git after running it.

Tests (tag round-trips against generated files, live LRCLIB lookups, console utility rendering and parsing):
```bash
DEEPKICK_TEST_DIR=/path/to/testfiles cargo test --manifest-path src-tauri/Cargo.toml --workspace --exclude tauri-plugin-deepkick -- --include-ignored
```
Test files can be generated with ffmpeg, e.g.
`ffmpeg -f lavfi -i "sine=frequency=440:duration=4" -metadata artist=Radiohead -metadata title=Creep -c:a libmp3lame creep.mp3`.
The robustness tests additionally want `junk.mp3` (1.5 KB of zeros between the ID3v2 tag and the audio), `year.mp3` (a `TYER` frame holding `1997 / 1998`), `cp1251.mp3` (artist `Дельфин` and title `Сны` as CP1251 bytes in Latin-1 frames), `exthdr.mp3` (extended-header flag set, no extended header) and `dli.mp3` (v2.4 frames flagged with a data length indicator they do not have, plus comments with a `\0\0\0` language); they are skipped when a fixture is missing. `cargo run -p deepkick-core --example inspect -- [--keep|--set] FILE` dumps what lofty makes of a file.
