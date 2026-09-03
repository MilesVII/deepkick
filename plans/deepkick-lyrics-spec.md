# DeepKick Lyrics - Technical Specification

## Overview
Android app (Tauri v2) for downloading and embedding song lyrics into MP3 and FLAC files.
- **Name**: DeepKick Lyrics
- **Package**: `com.milesseventh.deepkick`
- **Target**: Android 5.0+ (API 21+)
- **Stack**: Tauri 2, Rust backend, Vanilla TypeScript frontend with Rampike components

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (TypeScript)                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Units     │  │ Components  │  │  Services   │          │
│  │ (main,      │  │ (filepicker,│  │ (api,       │          │
│  │  queue,     │  │  modal,     │  │  storage,   │          │
│  │  settings)  │  │  tabs, etc) │  │  notify)    │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
└─────────┼────────────────┼────────────────┼─────────────────┘
          │                │                │
          ▼                ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                    Tauri IPC Layer                            │
│  invoke()  ◄──────────────────────────────────►  Commands    │
└─────────────────────────────────────────────────────────────┘
          │                                      │
          ▼                                      ▼
┌─────────────────────────┐         ┌─────────────────────────┐
│   Rust Backend          │         │   Android Platform      │
│  ┌───────────────────┐  │         │  ┌───────────────────┐  │
│  │ Metadata Reader   │  │         │  │ Foreground Service│  │
│  │ (MP3/FLAC)        │  │         │  │ + Notification    │  │
│  ├───────────────────┤  │         │  ├───────────────────┤  │
│  │ Metadata Writer   │  │         │  │ Storage Access    │  │
│  │ (USLT/LYRICS)     │  │         │  │ Framework         │  │
│  ├───────────────────┤  │         │  ├───────────────────┤  │
│  │ Lyrics Fetcher    │  │         │  │ MediaStore/       │  │
│  │ (lrclib.net)      │  │         │  │ FileProvider      │  │
│  ├───────────────────┤  │         │  └───────────────────┘  │
│  │ Batch Processor   │  │         └─────────────────────────┘
│  └───────────────────┘  │
└─────────────────────────┘
```

---

## Data Models

### Track (Frontend ↔ Backend)
```typescript
interface Track {
  id: string;                    // Unique identifier (path hash)
  path: string;                  // File path or content URI
  name: string;                  // Filename
  format: 'mp3' | 'flac';
  metadata: TrackMetadata;
  lyrics: LyricsStatus;
  processingState: 'pending' | 'processing' | 'done' | 'error';
  errorMessage?: string;
}

interface TrackMetadata {
  title: string;
  artist: string;
  album?: string;
  year?: number;
  trackNumber?: number;
  duration?: number;
}

interface LyricsStatus {
  hasLyrics: boolean;
  lyrics?: string;
  source?: string;               // API source URL
  synced: boolean;               // LRC vs plain text
}
```

### Processing Queue
```typescript
interface QueueState {
  tracks: Track[];
  stats: ProcessingStats;
  isProcessing: boolean;
  currentIndex: number;
}

interface ProcessingStats {
  total: number;
  processed: number;
  found: number;
  notFound: number;
  errors: number;
  skipped: number;               // Already had lyrics
}
```

### API Response (lrclib.net)
```typescript
interface LrcLibResponse {
  id: number;
  trackName: string;
  artistName: string;
  albumName: string;
  duration: number;
  instrumental: boolean;
  plainLyrics: string | null;
  syncedLyrics: string | null;
}
```

---

## Rust Backend Implementation

### Dependencies (Cargo.toml)
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }
lofty = "0.30"                    # Audio metadata (MP3 ID3v2, FLAC Vorbis)
id3 = "0.6"                       # ID3v2 frame manipulation for USLT
metaflac = "0.9"                  # FLAC Vorbis comment editing
walkdir = "2"                     # Directory traversal
sha2 = "0.10"                     # Path hashing for track IDs
notify-rust = "4"                 # Desktop notifications (dev)
```

### Core Modules

#### 1. `metadata.rs` - Reading metadata
```rust
pub fn read_metadata(path: &Path) -> Result<TrackMetadata, MetadataError>
```
- MP3: Use `lofty` to read ID3v2 tags (TIT2, TPE1, TALB, TYER, TRCK, TLEN)
- FLAC: Use `lofty` to read Vorbis comments (TITLE, ARTIST, ALBUM, DATE, TRACKNUMBER)

#### 2. `lyrics_writer.rs` - Writing lyrics
```rust
pub fn write_lyrics(path: &Path, lyrics: &str, format: AudioFormat) -> Result<(), WriteError>
```
- MP3: Use `id3` crate to add/replace USLT frame (unsynchronized lyrics)
- FLAC: Use `metaflac` to set VORBIS_COMMENT `LYRICS` field

#### 3. `lyrics_fetcher.rs` - API integration
```rust
pub async fn fetch_lyrics(artist: &str, title: &str) -> Result<LyricsResult, FetchError>
```
- Primary: `https://lrclib.net/api/get?artist_name={}&track_name={}`
- Fallback 1: Search endpoint `https://lrclib.net/api/search?q={}`
- Fallback 2: MusicBrainz + other free APIs (optional)
- Return both plain and synced lyrics if available

#### 4. `batch_processor.rs` - Queue processing
```rust
pub struct BatchProcessor {
    tracks: Vec<TrackTask>,
    progress_tx: mpsc::Sender<ProgressEvent>,
    cancel_rx: mpsc::Receiver<()>,
}

pub enum ProgressEvent {
    Started { index: usize, path: String },
    Progress { index: usize, status: TrackStatus },
    Completed { stats: ProcessingStats },
}
```
- Process tracks sequentially
- Emit progress events via Tauri event system
- Handle cancellation
- Atomic file writes (temp file + rename)

---

## Tauri Commands

```rust
// File selection
#[tauri::command]
async fn pick_files() -> Result<Vec<FileInfo>, String>

#[tauri::command]
async fn pick_directory() -> Result<Vec<FileInfo>, String>

// Metadata
#[tauri::command]
async fn read_track_metadata(path: String) -> Result<TrackMetadata, String>

// Lyrics operations
#[tauri::command]
async fn fetch_lyrics(artist: String, title: String) -> Result<LyricsResult, String>

#[tauri::command]
async fn write_lyrics(path: String, lyrics: String) -> Result<(), String>

#[tauri::command]
async fn remove_lyrics(path: String) -> Result<(), String>

// Batch processing
#[tauri::command]
async fn start_batch_processing(tracks: Vec<TrackTask>) -> Result<String, String>  // Returns session ID

#[tauri::command]
async fn cancel_batch_processing(session_id: String) -> Result<(), String>

// Events (frontend listens)
#[tauri::command]
async fn on_batch_progress(callback: ...)  // Handled via Tauri event system
```

---

## Android-Specific Implementation

### Permissions (AndroidManifest.xml)
```xml
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
<uses-permission android:name="android.permission.READ_MEDIA_AUDIO" />
<!-- For Android 12L and below -->
<uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" 
                 android:maxSdkVersion="32" />
```

### Foreground Service (Kotlin)
```kotlin
// src-tauri/gen/android/app/src/main/java/.../LyricsForegroundService.kt
class LyricsForegroundService : Service() {
    // Show persistent notification with progress
    // Handle MediaStore URIs via ContentResolver
    // Communicate with Rust via Tauri plugin or JNI
}
```

### Tauri Android Plugin
Create a Tauri plugin for Android-specific functionality:
- `tauri-plugin-deepkick-android` with:
  - File picker using Storage Access Framework
  - Foreground service management
  - MediaStore integration for reading/writing audio files
  - Notification channel creation

---

## Frontend Architecture (Following Aegir Pattern)

### Project Structure
```
src/
├── index.ts                 # Entry point, defines custom elements, initializes units
├── types.ts                 # Shared TypeScript interfaces
├── utils.ts                 # Helper functions
├── persist.ts               # IndexedDB/localStorage for settings/history
├── api.ts                   # Tauri invoke wrappers
├── components/
│   ├── filepicker.ts        # Rampike-based file/directory picker
│   ├── modal.ts             # Modal dialogs
│   ├── tabs.ts              # Tab navigation
│   ├── track-list.ts        # Queue display with progress
│   ├── lyrics-editor.ts     # Manual lyrics editing
│   └── toast.ts             # Notifications
├── units/
│   ├── main.ts              # App shell, navigation
│   ├── queue.ts             # Main processing queue UI
│   ├── settings.ts          # API keys, preferences
│   └── history.ts           # Processing history/reports
└── styles/
    ├── main.css
    ├── components.css
    └── theme.css
```

### Key Components

#### `dk-file-picker` (Custom Element)
- Wraps `<input type="file" multiple accept="audio/mpeg,audio/flac,.mp3,.flac">`
- Directory picker via `webkitdirectory` attribute
- Falls back to Tauri `pick_files`/`pick_directory` commands on Android

#### `dk-track-list` (Custom Element)
- Displays queue with: filename, artist/title, format, lyrics status, progress
- Row actions: edit lyrics, remove from queue, retry
- Batch actions: start processing, clear queue, remove all lyrics

#### `dk-lyrics-editor` (Modal)
- Textarea for plain lyrics
- Optional: LRC timestamp editor
- Save → calls `write_lyrics` command

---

## Lyrics Fetching Strategy

### Primary: lrclib.net
```
GET https://lrclib.net/api/get?artist_name={artist}&track_name={title}
Response: { plainLyrics, syncedLyrics, ... }
```

### Fallback: Search
```
GET https://lrclib.net/api/search?q={artist} {title}
Returns array, pick best match by duration/artist similarity
```

### Matching Algorithm
1. Exact match on artist + title
2. Fuzzy match (Levenshtein distance < 3)
3. Duration-based filtering (±5 seconds)
4. Prefer synced lyrics (LRC) over plain

---

## Foreground Service & Notifications

### Notification Channel
- ID: `lyrics_processing`
- Name: "Lyrics Processing"
- Importance: LOW (persistent but non-intrusive)

### Notification Content
- Title: "DeepKick Lyrics: Processing..."
- Progress bar: current/total
- Text: "Processing: {artist} - {title} ({current}/{total})"
- Actions: Cancel, Pause (optional)

### Completion Notification
- Title: "Processing Complete"
- Text: "Found: X, Not Found: Y, Errors: Z"
- Action: "View Report" → opens app to history screen

---

## Settings & Persistence

### Stored in IndexedDB (frontend) / SharedPreferences (Android)
```typescript
interface Settings {
  apiEndpoint: string;           // Default: lrclib.net
  fallbackEndpoints: string[];
  preferSyncedLyrics: boolean;
  autoRemoveExisting: boolean;   // Skip files with existing lyrics
  maxConcurrentFetches: number;  // Always 1 for sequential
  notificationOnComplete: boolean;
  theme: 'light' | 'dark' | 'system';
}
```

---

## Error Handling

| Scenario | Handling |
|----------|----------|
| No metadata (tags) | Skip, mark as "No Tags" |
| Lyrics not found | Mark as "Not Found", continue |
| Network error | Retry 3x with exponential backoff |
| Write permission denied | Show error, suggest SAF picker |
| Corrupted audio file | Skip, log error |
| Foreground service killed | Persist queue state, resume on restart |

---

## Testing Checklist

- [ ] MP3 ID3v2.3/2.4 USLT writing
- [ ] FLAC Vorbis LYRICS writing
- [ ] lrclib.net API integration
- [ ] Android SAF file picking (multiple files + directories)
- [ ] Foreground service survives app backgrounding
- [ ] Notification updates in real-time
- [ ] Large queue (100+ files) performance
- [ ] Special characters in metadata (Unicode)
- [ ] Files with existing lyrics (overwrite/skip)
- [ ] Cancellation mid-processing
- [ ] App restart during processing (state recovery)

---

## Build & Distribution

### Development
```bash
npm run dev          # Vite dev server
npm run tauri dev    # Tauri Android dev (requires device/emulator)
```

### Production
```bash
npm run tauri build  # Generates .aab for Play Store
```

### Signing
Configure `src-tauri/gen/android/app/build.gradle.kts` with signing config for release builds.

---

## Future Enhancements (Post-MVP)

1. **Synced lyrics (LRC) support** - Display with playback
2. **Multiple API providers** - Genius, Musixmatch, etc.
3. **Batch edit metadata** - Fix tags before fetching
4. **Cloud sync** - Backup lyrics database
5. **Widget** - Quick access to recent processing
6. **Auto-fetch on file add** - MediaStore observer