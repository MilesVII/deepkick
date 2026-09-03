import { loadSettings, saveSettings, DEFAULT_SETTINGS } from '../persist';
import type { Settings } from '../types';
import { toastSuccess } from '../components/toast';

export function settingsUnit(): void {
  const form = document.getElementById('settings-form') as HTMLFormElement;
  if (!form) return;

  loadAndPopulateSettings();

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const settings = collectSettings();
    await saveSettings(settings);
    toastSuccess('Settings saved');
  });

  // Reset to defaults
  const resetBtn = document.getElementById('btn-reset-settings');
  resetBtn?.addEventListener('click', async () => {
    await saveSettings(DEFAULT_SETTINGS);
    loadAndPopulateSettings();
    toastSuccess('Settings reset to defaults');
  });
}

async function loadAndPopulateSettings(): Promise<void> {
  const settings = await loadSettings();
  populateForm(settings);
}

function populateForm(settings: Settings): void {
  const apiEndpoint = document.getElementById('setting-api-endpoint') as HTMLInputElement | null;
  const preferSynced = document.getElementById('setting-prefer-synced') as HTMLInputElement | null;
  const autoRemove = document.getElementById('setting-auto-remove') as HTMLInputElement | null;
  const notifications = document.getElementById('setting-notifications') as HTMLInputElement | null;
  const theme = document.getElementById('setting-theme') as HTMLSelectElement | null;

  if (apiEndpoint) apiEndpoint.value = settings.apiEndpoint;
  if (preferSynced) preferSynced.checked = settings.preferSyncedLyrics;
  if (autoRemove) autoRemove.checked = settings.autoRemoveExisting;
  if (notifications) notifications.checked = settings.notificationOnComplete;
  if (theme) theme.value = settings.theme;
}

function collectSettings(): Settings {
  const apiEndpoint = (document.getElementById('setting-api-endpoint') as HTMLInputElement | null)?.value || DEFAULT_SETTINGS.apiEndpoint;
  const preferSynced = (document.getElementById('setting-prefer-synced') as HTMLInputElement | null)?.checked ?? DEFAULT_SETTINGS.preferSyncedLyrics;
  const autoRemove = (document.getElementById('setting-auto-remove') as HTMLInputElement | null)?.checked ?? DEFAULT_SETTINGS.autoRemoveExisting;
  const notifications = (document.getElementById('setting-notifications') as HTMLInputElement | null)?.checked ?? DEFAULT_SETTINGS.notificationOnComplete;
  const theme = (document.getElementById('setting-theme') as HTMLSelectElement | null)?.value as Settings['theme'] || DEFAULT_SETTINGS.theme;

  return {
    apiEndpoint,
    fallbackEndpoints: [],
    preferSyncedLyrics: preferSynced,
    autoRemoveExisting: autoRemove,
    notificationOnComplete: notifications,
    theme,
  };
}