import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  TrackInfo,
  TrackTask,
  LyricsResult,
  BatchResult,
  FileInfo,
  ProcessingProgress,
} from './types';
export type {
  TrackInfo,
  TrackTask,
  LyricsResult,
  BatchResult,
  FileInfo,
  ProcessingProgress,
};

// File operations
export async function pickFiles(): Promise<FileInfo[]> {
  return invoke('pick_files');
}

export async function pickDirectory(): Promise<FileInfo[]> {
  return invoke('pick_directory');
}

export async function scanDirectoryForAudio(path: string): Promise<FileInfo[]> {
  return invoke('scan_directory_for_audio', { path });
}

// Metadata operations
export async function readMetadata(path: string): Promise<TrackInfo> {
  return invoke('read_metadata', { path });
}

// Lyrics operations
export async function fetchLyrics(artist: string, title: string): Promise<LyricsResult> {
  return invoke('fetch_lyrics', { artist, title });
}

export async function writeLyrics(path: string, lyrics: string): Promise<void> {
  return invoke('write_lyrics_cmd', { path, lyrics });
}

export async function removeLyrics(path: string): Promise<void> {
  return invoke('remove_lyrics_cmd', { path });
}

// Batch processing
export async function startBatchProcessing(tasks: TrackTask[]): Promise<string> {
  return invoke('start_batch_processing', { tasks });
}

export async function cancelBatchProcessing(sessionId: string): Promise<void> {
  return invoke('cancel_batch_processing', { sessionId });
}

export async function removeLyricsBatch(tasks: TrackTask[]): Promise<BatchResult> {
  return invoke('remove_lyrics_batch_cmd', { tasks });
}

// Event listeners for batch progress
export type ProgressCallback = (progress: ProcessingProgress) => void;
export type CompleteCallback = (result: BatchResult) => void;
export type ErrorCallback = (error: string) => void;

export async function onBatchProgress(
  sessionId: string,
  callback: ProgressCallback
): Promise<UnlistenFn> {
  return listen<ProcessingProgress>(`batch-progress-${sessionId}`, (event) => {
    callback(event.payload);
  });
}

export async function onBatchComplete(
  sessionId: string,
  callback: CompleteCallback
): Promise<UnlistenFn> {
  return listen<BatchResult>(`batch-complete-${sessionId}`, (event) => {
    callback(event.payload);
  });
}

export async function onBatchError(
  sessionId: string,
  callback: ErrorCallback
): Promise<UnlistenFn> {
  return listen<string>(`batch-error-${sessionId}`, (event) => {
    callback(event.payload);
  });
}

// Utility: generate track ID from path
export function generateTrackId(path: string): string {
  let hash = 0;
  for (let i = 0; i < path.length; i++) {
    const char = path.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash;
  }
  return Math.abs(hash).toString(16).padStart(8, '0');
}

// Utility: create TrackInfo from FileInfo
export function createTrackInfo(file: FileInfo): TrackInfo {
  const format = file.name.toLowerCase().endsWith('.flac') ? 'flac' : 'mp3';
  return {
    id: generateTrackId(file.path),
    path: file.path,
    name: file.name,
    format,
    metadata: {
      title: '',
      artist: '',
    },
    lyricsStatus: {
      hasLyrics: false,
      synced: false,
    },
    processingState: 'pending',
  };
}