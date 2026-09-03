import { mudcrack } from 'rampike';

export class DkFilePicker extends HTMLElement {
  private input: HTMLInputElement;
  private onChangeCallback: ((files: File[]) => void) | null = null;

  constructor() {
    super();

    this.input = mudcrack({
      tagName: 'input',
      attributes: {
        type: 'file',
        accept: 'audio/mpeg,audio/flac,.mp3,.flac',
        multiple: '',
      },
      style: {
        display: 'none',
      },
    });

    this.input.addEventListener('change', (e: Event) => {
      const target = e.target as HTMLInputElement;
      if (target.files && this.onChangeCallback) {
        this.onChangeCallback(Array.from(target.files));
      }
      target.value = '';
    });

    const label = mudcrack({
      tagName: 'label',
      style: {
        display: 'contents',
      },
      contents: [
        this.input,
        ...Array.from(this.children),
      ],
    });

    this.append(label);
  }

  set onChange(callback: (files: File[]) => void) {
    this.onChangeCallback = callback;
  }

  pickFiles(): void {
    this.input.click();
  }

  pickDirectory(): void {
    const dirInput = document.createElement('input');
    dirInput.type = 'file';
    dirInput.webkitdirectory = true;
    dirInput.accept = 'audio/mpeg,audio/flac,.mp3,.flac';
    dirInput.onchange = (e: Event) => {
      const target = e.target as HTMLInputElement;
      if (target.files && this.onChangeCallback) {
        this.onChangeCallback(Array.from(target.files));
      }
    };
    dirInput.click();
  }
}

export function define(tagName: string = 'dk-file-picker'): void {
  if (!customElements.get(tagName)) {
    customElements.define(tagName, DkFilePicker);
  }
}

export type DkFilePickerElement = DkFilePicker;