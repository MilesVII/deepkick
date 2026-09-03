import { readMetadata, startBatchProcessing, cancelBatchProcessing, onBatchProgress, onBatchComplete, onBatchError, createTrackInfo, type TrackInfo, type TrackTask, type ProcessingProgress, type BatchResult } from '../api';
import { toastSuccess, toastError, toastInfo } from '../components/toast';
import type { DkTrackListElement } from '../components/tracklist';
import type { DkModalElement } from '../components/modal';
import type { DkLyricsEditorElement } from '../components/lyrics-editor';

let trackList: DkTrackListElement | null = null;
let lyricsModal: DkModalElement | null = null;
let lyricsEditor: DkLyricsEditorElement | null = null;
let currentSessionId: string | null = null;
let progressUnlisten: (() => void) | null = null;
let completeUnlisten: (() => void) | null = null;
let errorUnlisten: (() => void) | null = null;

export function queueUnit(): void {
  trackList = document.querySelector('dk-track-list') as DkTrackListElement;
  lyricsModal = document.querySelector('dk-modal') as DkModalElement;
  lyricsEditor = document.querySelector('dk-lyrics-editor') as DkLyricsEditorElement;

  if (!trackList || !lyricsModal || !lyricsEditor) {
    console.warn('Queue unit: required components not found');
    return;
  }

  setupFilePickers();
  setupTrackList();
  setupLyricsEditor();
  setupBatchActions();
}

function setupFilePickers(): void {
  const filePicker = document.querySelector('dk-file-picker[data-action="files"]');
  const dirPicker = document.querySelector('dk-file-picker[data-action="directory"]');

  filePicker?.addEventListener('change', async (e: Event) => {
    const files = (e as CustomEvent).detail as File[];
    await addFilesFromFileList(files);
  });

  dirPicker?.addEventListener('change', async (e: Event) => {
    const files = (e as CustomEvent).detail as File[];
    await addFilesFromFileList(files);
  });

  // Also handle native file input events
  const fileInput = filePicker?.querySelector('input[type="file"]');
  const dirInput = dirPicker?.querySelector('input[type="file"]');

  fileInput?.addEventListener('change', async (e: Event) => {
    const target = e.target as HTMLInputElement;
    if (target.files) {
      await addFilesFromFileList(Array.from(target.files));
      target.value = '';
    }
  });

  dirInput?.addEventListener('change', async (e: Event) => {
    const target = e.target as HTMLInputElement;
    if (target.files) {
      await addFilesFromFileList(Array.from(target.files));
      target.value = '';
    }
  });
}

async function addFilesFromFileList(files: File[]): Promise<void> {
  const audioFiles = files.filter(f => f.name.toLowerCase().endsWith('.mp3') || f.name.toLowerCase().endsWith('.flac'));

  if (audioFiles.length === 0) {
    toastInfo('No MP3 or FLAC files selected');
    return;
  }

  for (const file of audioFiles) {
    try {
      // For web, we can't directly read file paths, so we use the file name
      // On Android, the Tauri commands will handle actual file paths
      const track = createTrackInfo({
        path: file.name, // This will be replaced by actual path on Android
        name: file.name,
        isDirectory: false,
        size: file.size,
      });

      // Try to read metadata if possible
      try {
        const metadata = await readMetadata(file.name);
        track.metadata = metadata.metadata;
        track.lyricsStatus = metadata.lyricsStatus;
      } catch {
        // Metadata reading might fail on web, that's okay
      }

      trackList?.updateTrack?.(track) || trackList?.setTracks([...(trackList.getTracks?.() || []), track]);
    } catch (e) {
      console.error('Failed to add file:', e);
    }
  }

  toastSuccess(`Added ${audioFiles.length} file(s) to queue`);
}

function setupTrackList(): void {
  if (!trackList) return;

  trackList.onTrackActionCallback = async (action, track) => {
    switch (action) {
      case 'toggle':
        track.lyricsStatus.hasLyrics = !track.lyricsStatus.hasLyrics;
        trackList?.updateTrack(track);
        break;
      case 'edit':
        openLyricsEditor(track);
        break;
      case 'retry':
        await retryTrack(track);
        break;
      case 'remove':
        removeTrack(track);
        break;
    }
  };
}

function setupLyricsEditor(): void {
  const editor = lyricsEditor;
  const modal = lyricsModal;
  if (!editor || !modal) return;

  editor.onSave = async (lyrics) => {
    const track = editor.getTrack();
    if (!track) return;

    try {
      await writeLyricsToFile(track, lyrics);
      track.lyricsStatus.hasLyrics = true;
      track.lyricsStatus.lyrics = lyrics;
      track.lyricsStatus.synced = lyrics.includes('[') && lyrics.includes(']');
      trackList?.updateTrack(track);
      toastSuccess('Lyrics saved successfully');
    } catch (e) {
      toastError(`Failed to save lyrics: ${e}`);
    }
    modal.hide();
  };

  editor.onCancel = () => {
    modal.hide();
  };
}

function openLyricsEditor(track: TrackInfo): void {
  if (!lyricsEditor || !lyricsModal) return;
  lyricsEditor.setTrack(track);
  lyricsModal.show();
}

async function writeLyricsToFile(track: TrackInfo, lyrics: string): Promise<void> {
  // This will call the Tauri command
  const { writeLyrics } = await import('../api');
  await writeLyrics(track.path, lyrics);
}

async function retryTrack(track: TrackInfo): Promise<void> {
  track.processingState = 'pending';
  track.errorMessage = undefined;
  trackList?.updateTrack(track);

  // Re-process this single track
  const task: TrackTask = {
    track,
    overwriteExisting: true,
  };

  try {
    const sessionId = await startBatchProcessing([task]);
    currentSessionId = sessionId;
    setupBatchListeners(sessionId);
  } catch (e) {
    track.processingState = 'error';
    track.errorMessage = String(e);
    trackList?.updateTrack(track);
    toastError(`Retry failed: ${e}`);
  }
}

function removeTrack(track: TrackInfo): void {
  const tracks = trackList?.getTracks?.() || [];
  trackList?.setTracks(tracks.filter(t => t.id !== track.id));
  toastInfo('Track removed from queue');
}

function setupBatchActions(): void {
  const startBtn = document.getElementById('btn-start-processing');
  const cancelBtn = document.getElementById('btn-cancel-processing');
  const clearBtn = document.getElementById('btn-clear-queue');
  const removeLyricsBtn = document.getElementById('btn-remove-lyrics');

  startBtn?.addEventListener('click', startProcessing);
  cancelBtn?.addEventListener('click', cancelProcessing);
  clearBtn?.addEventListener('click', clearQueue);
  removeLyricsBtn?.addEventListener('click', removeAllLyrics);
}

async function startProcessing(): Promise<void> {
  if (!trackList) return;

  const tracks = trackList.getTracks();
  const pendingTracks = tracks.filter(t => !t.lyricsStatus.hasLyrics || t.processingState === 'error');

  if (pendingTracks.length === 0) {
    toastInfo('No tracks to process');
    return;
  }

  const tasks: TrackTask[] = pendingTracks.map(track => ({
    track,
    overwriteExisting: true,
  }));

  try {
    const sessionId = await startBatchProcessing(tasks);
    currentSessionId = sessionId;
    setupBatchListeners(sessionId);
    toastInfo('Processing started...');
  } catch (e) {
    toastError(`Failed to start processing: ${e}`);
  }
}

function setupBatchListeners(sessionId: string): void {
  // Clean up previous listeners
  progressUnlisten?.();
  completeUnlisten?.();
  errorUnlisten?.();

  onBatchProgress(sessionId, handleProgress).then(unlisten => progressUnlisten = unlisten);
  onBatchComplete(sessionId, handleComplete).then(unlisten => completeUnlisten = unlisten);
  onBatchError(sessionId, handleError).then(unlisten => errorUnlisten = unlisten);
}

function handleProgress(progress: ProcessingProgress): void {
  if (progress.currentTrack) {
    trackList?.updateTrack(progress.currentTrack);
  }

  // Update progress UI
  const progressBar = document.getElementById('processing-progress') as HTMLProgressElement;
  const progressText = document.getElementById('processing-text');
  if (progressBar) {
    progressBar.max = progress.total;
    progressBar.value = progress.currentIndex;
  }
  if (progressText) {
    progressText.textContent = `Processing ${progress.currentIndex}/${progress.total}`;
  }
}

function handleComplete(result: BatchResult): void {
  toastSuccess(`Processing complete: ${result.stats.found} found, ${result.stats.notFound} not found, ${result.stats.errors} errors`);

  // Update all tracks with results
  result.tracks.forEach((track: TrackInfo) => {
    track.processingState = track.lyricsStatus.hasLyrics ? 'done' : 'error';
    trackList?.updateTrack(track);
  });

  // Reset progress UI
  const progressBar = document.getElementById('processing-progress') as HTMLProgressElement;
  const progressText = document.getElementById('processing-text');
  if (progressBar) progressBar.value = progressBar.max;
  if (progressText) progressText.textContent = 'Complete';

  currentSessionId = null;
}

function handleError(error: string): void {
  toastError(`Processing error: ${error}`);
  currentSessionId = null;
}

async function cancelProcessing(): Promise<void> {
  if (currentSessionId) {
    await cancelBatchProcessing(currentSessionId);
    toastInfo('Processing cancelled');
    currentSessionId = null;
  }
}

function clearQueue(): void {
  trackList?.setTracks([]);
  toastInfo('Queue cleared');
}

async function removeAllLyrics(): Promise<void> {
  if (!trackList) return;

  const tracks = trackList.getTracks();
  const tracksWithLyrics = tracks.filter(t => t.lyricsStatus.hasLyrics);

  if (tracksWithLyrics.length === 0) {
    toastInfo('No tracks with lyrics to remove');
    return;
  }

  const tasks: TrackTask[] = tracksWithLyrics.map(track => ({
    track,
    overwriteExisting: true,
  }));

  try {
    const { removeLyricsBatch } = await import('../api');
    const result = await removeLyricsBatch(tasks);

    result.tracks.forEach(track => {
      track.lyricsStatus.hasLyrics = false;
      track.lyricsStatus.lyrics = undefined;
      track.lyricsStatus.source = undefined;
      track.lyricsStatus.synced = false;
      trackList?.updateTrack(track);
    });

    toastSuccess(`Removed lyrics from ${result.stats.found} track(s)`);
  } catch (e) {
    toastError(`Failed to remove lyrics: ${e}`);
  }
}