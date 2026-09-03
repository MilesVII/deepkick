import type { TrackInfo } from '../types';

export class DkLyricsEditor extends HTMLElement {
  private textarea: HTMLTextAreaElement;
  private track: TrackInfo | null = null;
  private onSaveCallback: ((lyrics: string) => void) | null = null;
  private onCancelCallback: (() => void) | null = null;

  constructor() {
    super();

    this.textarea = document.createElement('textarea');
    this.textarea.style.cssText = `
      width:100%;min-height:300px;padding:12px;border:1px solid var(--border,#e0e0e0);
      border-radius:8px;font-family:monospace;font-size:14px;line-height:1.5;
      resize:vertical;box-sizing:border-box;
    `;
    this.textarea.placeholder = 'Enter lyrics here...';

    const header = document.createElement('div');
    header.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:16px;';
    header.innerHTML = '<h3 style="margin:0;">Edit Lyrics</h3>';

    const actions = document.createElement('div');
    actions.style.cssText = 'display:flex;gap:8px;justify-content:flex-end;margin-top:16px;';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dk-btn dk-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.style.cssText = 'padding:8px 16px;';
    cancelBtn.addEventListener('click', () => this.onCancelCallback?.());

    const saveBtn = document.createElement('button');
    saveBtn.className = 'dk-btn dk-btn-primary';
    saveBtn.textContent = 'Save';
    saveBtn.style.cssText = 'padding:8px 16px;';
    saveBtn.addEventListener('click', () => this.onSaveCallback?.(this.textarea.value));

    actions.append(cancelBtn, saveBtn);

    this.append(header, this.textarea, actions);
  }

  setTrack(track: TrackInfo): void {
    this.track = track;
    this.textarea.value = track.lyricsStatus.lyrics || '';
  }

  getTrack(): TrackInfo | null {
    return this.track;
  }

  getLyrics(): string {
    return this.textarea.value;
  }

  set onSave(callback: (lyrics: string) => void) {
    this.onSaveCallback = callback;
  }

  set onCancel(callback: () => void) {
    this.onCancelCallback = callback;
  }
}

export function define(tagName: string = 'dk-lyrics-editor'): void {
  if (!customElements.get(tagName)) {
    customElements.define(tagName, DkLyricsEditor);
  }
}

export type DkLyricsEditorElement = DkLyricsEditor;