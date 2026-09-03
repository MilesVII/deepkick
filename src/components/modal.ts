import { mudcrack } from 'rampike';

export class DkModal extends HTMLElement {
  private overlay: HTMLElement;
  private content: HTMLElement;
  private onCloseCallback: (() => void) | null = null;
  private _isOpen = false;

  constructor() {
    super();
    this.attachShadow({ mode: 'open' });

    this.overlay = mudcrack({
      tagName: 'div',
      className: 'dk-modal-overlay',
      style: {
        position: 'fixed',
        inset: '0',
        background: 'rgba(0, 0, 0, 0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: '1000',
        opacity: '0',
        visibility: 'hidden',
        transition: 'opacity 0.2s, visibility 0.2s',
      },
    });

    this.content = mudcrack({
      tagName: 'div',
      className: 'dk-modal-content',
      style: {
        background: 'var(--surface, white)',
        borderRadius: '12px',
        padding: '24px',
        maxWidth: '600px',
        width: '90%',
        maxHeight: '80vh',
        overflow: 'auto',
        transform: 'scale(0.95)',
        transition: 'transform 0.2s',
      },
    });

    this.overlay.addEventListener('click', () => this.hide());
    this.content.addEventListener('click', (e: Event) => e.stopPropagation());

    this.overlay.appendChild(this.content);
    this.shadowRoot!.appendChild(this.overlay);

    // Add styles
    const style = document.createElement('style');
    style.textContent = `
      .dk-modal-overlay.open {
        opacity: 1;
        visibility: visible;
      }
      .dk-modal-overlay.open .dk-modal-content {
        transform: scale(1);
      }
      ::slotted(*) {
        display: block;
      }
    `;
    this.shadowRoot!.appendChild(style);
  }

  show(): void {
    this._isOpen = true;
    this.overlay.classList.add('open');
    document.body.style.overflow = 'hidden';
  }

  hide(): void {
    this._isOpen = false;
    this.overlay.classList.remove('open');
    document.body.style.overflow = '';
    if (this.onCloseCallback) {
      this.onCloseCallback();
    }
  }

  setContent(html: string | HTMLElement): void {
    if (typeof html === 'string') {
      this.content.innerHTML = html;
    } else {
      this.content.innerHTML = '';
      this.content.appendChild(html);
    }
  }

  getContent(): HTMLElement {
    return this.content;
  }

  set onClose(callback: () => void) {
    this.onCloseCallback = callback;
  }

  get isOpen(): boolean {
    return this._isOpen;
  }
}

export function define(tagName: string = 'dk-modal'): void {
  if (!customElements.get(tagName)) {
    customElements.define(tagName, DkModal);
  }
}

export type DkModalElement = DkModal;