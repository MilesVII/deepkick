import { define as defineFilePicker } from '../components/filepicker';
import { define as defineModal } from '../components/modal';
import { define as defineTrackList } from '../components/tracklist';
import { define as defineLyricsEditor } from '../components/lyrics-editor';
import { queueUnit } from './queue';
import { settingsUnit } from './settings';
import { historyUnit } from './history';

defineFilePicker('dk-file-picker');
defineModal('dk-modal');
defineTrackList('dk-track-list');
defineLyricsEditor('dk-lyrics-editor');

export function mainUnit(): void {
  // Initialize all units
  queueUnit();
  settingsUnit();
  historyUnit();

  // Handle tab navigation
  const tabButtons = document.querySelectorAll('[data-tab]');
  const tabPanels = document.querySelectorAll('[data-tab-panel]');

  tabButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const tab = btn.getAttribute('data-tab');
      if (!tab) return;

      tabButtons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      tabPanels.forEach(panel => {
        panel.classList.toggle('active', panel.getAttribute('data-tab-panel') === tab);
      });
    });
  });

  // Theme handling
  const themeSelect = document.getElementById('theme-select') as HTMLSelectElement;
  if (themeSelect) {
    themeSelect.addEventListener('change', () => {
      document.documentElement.setAttribute('data-theme', themeSelect.value);
      localStorage.setItem('theme', themeSelect.value);
    });

    // Load saved theme
    const savedTheme = localStorage.getItem('theme') || 'system';
    themeSelect.value = savedTheme;
    document.documentElement.setAttribute('data-theme', savedTheme);
  }
}