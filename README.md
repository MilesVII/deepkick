# DeepKick Lyrics
Puts lyrics into MP3 and FLAC metadata. Android app built with Tauri 2, a vanilla TS frontend and Rust.

Successor of *Seventh Lyrics Manager*, which stopped working once Android moved to the Storage Access Framework.

## What it does
- Pick files or whole folders through the system picker; supported files (`.mp3`, `.flac`) end up in a queue
- **Fetch lyrics**: a foreground service keeps the process alive while a Rust worker walks the queue, looks each track up on [LRCLIB](https://lrclib.net) (optionally falling back to lyrics.ovh) by artist / title / album / duration, and writes the result into the `USLT` frame (ID3v2) or the `LYRICS` Vorbis comment (FLAC)
- **Remove lyrics** from every file in the queue
- A notification is posted when the run is over; tapping it opens the **report** with the status of every file
- Tap any file to open the **editor**: view, edit, search online (pick between plain and synced candidates), save or remove its lyrics

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
    tags.rs           lofty-based lyrics read/write for MP3 and FLAC
    lyrics.rs         LRCLIB + lyrics.ovh client (ureq)
    persist.rs        queue + report persisted in the app data dir
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

Core tests (tag round-trips against generated files, live LRCLIB lookups):
```bash
DEEPKICK_TEST_DIR=/path/to/testfiles cargo test --manifest-path src-tauri/Cargo.toml --lib -- --include-ignored
```
Test files can be generated with ffmpeg, e.g.
`ffmpeg -f lavfi -i "sine=frequency=440:duration=4" -metadata artist=Radiohead -metadata title=Creep -c:a libmp3lame creep.mp3`.

## Prompt
```
hi! i want to make an app that allows user to download and embed song lyrics into user's MP3 and FLAC files. the name is DeepKick Lyrics.

# the stack
tauri, android only. the UI is made with vanilla web stack and TS.  the current project is initialized for tauri android development, refer to `package.json` for details.

# references
there is `./references` folder that has two projects included:
- slm: a very old version of the app that does the same thing. that app was called Seventh Lyrics Manager. it's a native android Java app that doesn't work with Android 5.0+ because of its new Storage Access Framework. you can use it to check some implementation details.
- aegir: it's a recent project, a roleplay frontend that demonstrates how my rampike utility can be used and how i want the frontend of the project to be organized.

# how it's supposed to work
- The app asks user to select some files on local storage, either via some kind of implemented file explorer or a system dialog. the user should be able to pick multiple separate files and/or directories, which contents should be added to the task queue.
- Once user selected a list of MP3 and FLAC files, and started the process, the app should create a foreground process and fetch lyrics for each file one by one. The app should use artist and song name from file metadata in order to let the API to identify the song and if the lyrics are found, they need to be added to file's metadata, (`USLT` frame for ID3v2 and `LYRICS` for FLAC).
- Once the queue is finished, a notification appears, that allows to check the report -- how many of the songs were identified and processed.
- The app should also allow the user to manually edit the lyrics for a specific file, and batch remove them as well.
- You are free to use any free public API to fetch lyrics. `https://lrclib.net/`'s API is okay, multiple fallback APIs is nice to have but not required.
```
