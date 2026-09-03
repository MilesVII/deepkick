// Shared TypeScript types matching Rust backend

export interface TrackMetadata {
  title: string;
  artist: string;
  album?: string;
  year?: number;
  trackNumber?: number;
  duration?: number; // milliseconds
}

export interface LyricsResult {
  plainLyrics?: string;
  syncedLyrics?: string;
  sourceUrl: string;
  instrumental: boolean;
}

export type AudioFormat = 'mp3' | 'flac';

export interface LyricsStatus {
  hasLyrics: boolean;
  lyrics?: string;
  source?: string;
  synced: boolean;
}

export interface TrackInfo {
  id: string;
  path: string;
  name: string;
  format: AudioFormat;
  metadata: TrackMetadata;
  lyricsStatus: LyricsStatus;
  processingState?: 'pending' | 'processing' | 'done' | 'error';
  errorMessage?: string;
}

export interface TrackTask {
  track: TrackInfo;
  overwriteExisting: boolean;
}

export interface ProcessingProgress {
  sessionId: string;
  currentIndex: number;
  total: number;
  currentTrack?: TrackInfo;
  status: 'pending' | 'processing' | 'completed' | 'failed' | 'cancelled';
  stats: ProcessingStats;
}

export interface ProcessingStats {
  total: number;
  processed: number;
  found: number;
  notFound: number;
  errors: number;
  skipped: number;
}

export interface BatchResult {
  sessionId: string;
  stats: ProcessingStats;
  tracks: TrackInfo[];
}

export interface FileInfo {
  path: string;
  name: string;
  isDirectory: boolean;
  size?: number;
}

export interface Settings {
  apiEndpoint: string;
  fallbackEndpoints: string[];
  preferSyncedLyrics: boolean;
  autoRemoveExisting: boolean;
  notificationOnComplete: boolean;
  theme: 'light' | 'dark' | 'system';
}

export const DEFAULT_SETTINGS: Settings = {
  apiEndpoint: 'https://lrclib.net/api',
  fallbackEndpoints: [],
  preferSyncedLyrics: true,
  autoRemoveExisting: false,
  notificationOnComplete: true,
  theme: 'system',
};