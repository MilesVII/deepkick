# DeepKick Lyrics
Puts lyrics into MP3 and FLAC metadata

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
