use crate::error::{AppError, Result};
use crate::lyrics_fetcher::LyricsFetcher;
use crate::lyrics_writer::{write_lyrics, remove_lyrics};
use crate::metadata::read_track_info;
use crate::types::{
    BatchResult, LyricsStatus, ProcessingProgress, ProcessingStats, ProcessingStatus,
    TrackInfo, TrackTask,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

pub struct BatchProcessor {
    fetcher: LyricsFetcher,
    progress_tx: mpsc::UnboundedSender<ProcessingProgress>,
    cancel_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<()>>>>,
    session_id: String,
}

impl BatchProcessor {
    pub fn new(
        progress_tx: mpsc::UnboundedSender<ProcessingProgress>,
        cancel_rx: mpsc::UnboundedReceiver<()>,
    ) -> Self {
        Self {
            fetcher: LyricsFetcher::new(),
            progress_tx,
            cancel_rx: Arc::new(Mutex::new(Some(cancel_rx))),
            session_id: Uuid::new_v4().to_string(),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn process(&mut self, tasks: Vec<TrackTask>) -> Result<BatchResult> {
        let total = tasks.len();
        let mut stats = ProcessingStats {
            total,
            ..Default::default()
        };
        let mut processed_tracks = Vec::with_capacity(total);

        for (index, mut task) in tasks.into_iter().enumerate() {
            // Check for cancellation
            if self.is_cancelled().await {
                stats.processed = index;
                return Err(AppError::Cancelled);
            }

            // Update progress: started
            self.emit_progress(ProcessingProgress {
                session_id: self.session_id.clone(),
                current_index: index,
                total,
                current_track: Some(task.track.clone()),
                status: ProcessingStatus::Processing,
                stats: stats.clone(),
            });

            // Process single track
            let result = self.process_track(&mut task).await;

            match result {
                Ok(updated_track) => {
                    stats.processed += 1;
                    if updated_track.lyrics_status.has_lyrics {
                        stats.found += 1;
                    } else {
                        stats.not_found += 1;
                    }
                    processed_tracks.push(updated_track);
                }
                Err(e) => {
                    stats.processed += 1;
                    stats.errors += 1;
                    tracing::error!("Error processing track {}: {}", task.track.name, e);
                    processed_tracks.push(task.track);
                }
            }

            // Update progress: completed track
            self.emit_progress(ProcessingProgress {
                session_id: self.session_id.clone(),
                current_index: index + 1,
                total,
                current_track: Some(processed_tracks.last().unwrap().clone()),
                status: ProcessingStatus::Processing,
                stats: stats.clone(),
            });
        }

        // Final progress
        self.emit_progress(ProcessingProgress {
            session_id: self.session_id.clone(),
            current_index: total,
            total,
            current_track: None,
            status: ProcessingStatus::Completed,
            stats: stats.clone(),
        });

        Ok(BatchResult {
            session_id: self.session_id.clone(),
            stats,
            tracks: processed_tracks,
        })
    }

    async fn process_track(&self, task: &mut TrackTask) -> Result<TrackInfo> {
        let track = &mut task.track;

        // Re-read metadata to ensure we have latest
        let path = Path::new(&track.path);
        let fresh_info = read_track_info(path)?;
        track.metadata = fresh_info.metadata;
        track.format = fresh_info.format;

        // Check if already has lyrics and we're not overwriting
        if track.lyrics_status.has_lyrics && !task.overwrite_existing {
            return Ok(track.clone());
        }

        // Fetch lyrics
        let lyrics_result = self
            .fetcher
            .fetch_lyrics(&track.metadata.artist, &track.metadata.title)
            .await?;

        // Determine which lyrics to use (prefer synced)
        let has_synced = lyrics_result.synced_lyrics.is_some();
        let lyrics = lyrics_result.synced_lyrics
            .or(lyrics_result.plain_lyrics)
            .ok_or(AppError::LyricsNotFound)?;

        // Write lyrics to file
        write_lyrics(path, &lyrics, track.format.clone())?;

        // Update track info
        track.lyrics_status = LyricsStatus {
            has_lyrics: true,
            lyrics: Some(lyrics),
            source: Some(lyrics_result.source_url),
            synced: has_synced,
        };

        Ok(track.clone())
    }

    async fn is_cancelled(&self) -> bool {
        let mut guard = self.cancel_rx.lock().await;
        if let Some(rx) = guard.as_mut() {
            rx.try_recv().is_ok()
        } else {
            false
        }
    }

    fn emit_progress(&self, progress: ProcessingProgress) {
        let _ = self.progress_tx.send(progress);
    }
}

pub async fn remove_lyrics_batch(tasks: Vec<TrackTask>) -> Result<BatchResult> {
    let total = tasks.len();
    let mut stats = ProcessingStats {
        total,
        ..Default::default()
    };
    let mut processed_tracks = Vec::with_capacity(total);

    for (index, mut task) in tasks.into_iter().enumerate() {
        let track = &mut task.track;
        let path = Path::new(&track.path);

        match remove_lyrics(path, track.format.clone()) {
            Ok(_) => {
                track.lyrics_status = LyricsStatus {
                    has_lyrics: false,
                    lyrics: None,
                    source: None,
                    synced: false,
                };
                stats.processed += 1;
                stats.found += 1;
            }
            Err(e) => {
                stats.processed += 1;
                stats.errors += 1;
                tracing::error!("Error removing lyrics from {}: {}", track.name, e);
            }
        }

        processed_tracks.push(track.clone());
    }

    Ok(BatchResult {
        session_id: Uuid::new_v4().to_string(),
        stats,
        tracks: processed_tracks,
    })
}