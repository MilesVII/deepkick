import { define as defineFilePicker } from './components/filepicker';
import { define as defineModal } from './components/modal';
import { define as defineTrackList } from './components/tracklist';
import { define as defineLyricsEditor } from './components/lyrics-editor';
import { mainUnit } from './units/main';

defineFilePicker('dk-file-picker');
defineModal('dk-modal');
defineTrackList('dk-track-list');
defineLyricsEditor('dk-lyrics-editor');

window.addEventListener('DOMContentLoaded', () => {
  mainUnit();
});