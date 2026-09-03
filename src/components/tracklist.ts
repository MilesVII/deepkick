import type { TrackInfo } from '../types';

export class DkTrackList extends HTMLElement {
  private container: HTMLElement;
  private tracks: TrackInfo[] = [];
  private onTrackAction: ((action: string, track: TrackInfo) => void) | null = null;

  constructor() {
    super();

    this.container = document.createElement('div');
    this.container.className = 'dk-track-list';
    this.container.style.cssText = 'display:flex;flex-direction:column;gap:8px;';
    this.appendChild(this.container);
  }

  setTracks(tracks: TrackInfo[]): void {
    this.tracks = tracks;
    this.render();
  }

  getTracks(): TrackInfo[] {
    return this.tracks;
  }

  updateTrack(track: TrackInfo): void {
    const index = this.tracks.findIndex(t => t.id === track.id);
    if (index !== -1) {
      this.tracks[index] = track;
      this.renderTrack(track);
    }
  }

  private render(): void {
    this.container.innerHTML = '';
    this.tracks.forEach((track) => {
      this.container.appendChild(this.createTrackElement(track));
    });
  }

  private createTrackElement(track: TrackInfo): HTMLElement {
    const hasLyrics = track.lyricsStatus.hasLyrics;
    const processing = track.processingState === 'processing';
    const error = track.processingState === 'error';

    const row = document.createElement('div');
    row.className = 'dk-track-row';
    row.style.cssText = `
      display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:12px;
      padding:12px;background:var(--surface,white);border-radius:8px;
      border:1px solid var(--border,#e0e0e0);opacity:${processing ? '0.7' : '1'};
      transition:opacity 0.2s;
    `;

    // Checkbox
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = hasLyrics;
    checkbox.style.cssText = 'width:20px;height:20px;cursor:pointer;';
    checkbox.addEventListener('change', () => this.onTrackAction?.('toggle', track));

    // Track info
    const info = document.createElement('div');
    info.className = 'dk-track-info';
    info.style.cssText = 'display:flex;flex-direction:column;gap:4px;min-width:0;';

    const nameEl = document.createElement('div');
    nameEl.className = 'dk-track-name';
    nameEl.style.cssText = 'font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
    nameEl.textContent = track.name;

    const metaEl = document.createElement('div');
    metaEl.className = 'dk-track-meta';
    metaEl.style.cssText = 'font-size:12px;color:var(--text-secondary,#666);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
    metaEl.textContent = `${track.metadata.artist || 'Unknown Artist'} - ${track.metadata.title || 'Unknown Title'}`;

    info.append(nameEl, metaEl);

    // Actions
    const actions = document.createElement('div');
    actions.className = 'dk-track-actions';
    actions.style.cssText = 'display:flex;gap:8px;';

    // Status badge
    const statusEl = document.createElement('span');
    statusEl.className = `dk-track-status ${this.getStatusClass(track)}`;
    statusEl.style.cssText = 'padding:4px 8px;border-radius:12px;font-size:11px;font-weight:500;text-transform:uppercase;';
    statusEl.textContent = this.getStatusText(track);

    // Edit button
    const editBtn = document.createElement('button');
    editBtn.className = 'dk-btn dk-btn-secondary dk-btn-sm';
    editBtn.style.cssText = 'padding:6px 12px;font-size:12px;';
    editBtn.textContent = 'Edit';
    editBtn.addEventListener('click', () => this.onTrackAction?.('edit', track));

    // Retry button (only for errors)
    if (error) {
      const retryBtn = document.createElement('button');
      retryBtn.className = 'dk-btn dk-btn-primary dk-btn-sm';
      retryBtn.style.cssText = 'padding:6px 12px;font-size:12px;';
      retryBtn.textContent = 'Retry';
      retryBtn.addEventListener('click', () => this.onTrackAction?.('retry', track));
      actions.appendChild(retryBtn);
    }

    // Remove button
    const removeBtn = document.createElement('button');
    removeBtn.className = 'dk-btn dk-btn-ghost dk-btn-sm';
    removeBtn.style.cssText = 'padding:6px 12px;font-size:12px;color:var(--error,#d32f2f);';
    removeBtn.textContent = 'Remove';
    removeBtn.addEventListener('click', () => this.onTrackAction?.('remove', track));

    actions.append(statusEl, editBtn, removeBtn);
    row.append(checkbox, info, actions);
    return row;
  }

  private renderTrack(track: TrackInfo): void {
    const index = this.tracks.findIndex(t => t.id === track.id);
    if (index === -1) return;
    const newElement = this.createTrackElement(track);
    const oldElement = this.container.children[index];
    if (oldElement) {
      this.container.replaceChild(newElement, oldElement);
    }
  }

  private getStatusClass(track: TrackInfo): string {
    if (track.processingState === 'processing') return 'processing';
    if (track.processingState === 'error') return 'error';
    if (track.processingState === 'done') return 'done';
    if (track.lyricsStatus.hasLyrics) return 'has-lyrics';
    return 'pending';
  }

  private getStatusText(track: TrackInfo): string {
    if (track.processingState === 'processing') return 'Processing...';
    if (track.processingState === 'error') return `Error: ${track.errorMessage || 'Unknown'}`;
    if (track.processingState === 'done') return 'Done';
    if (track.lyricsStatus.hasLyrics) return 'Has Lyrics';
    return 'Pending';
  }

  set onTrackActionCallback(callback: (action: string, track: TrackInfo) => void) {
    this.onTrackAction = callback;
  }
}

export function define(tagName: string = 'dk-track-list'): void {
  if (!customElements.get(tagName)) {
    customElements.define(tagName, DkTrackList);
  }
}

export type DkTrackListElement = DkTrackList;