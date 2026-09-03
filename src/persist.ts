import { DEFAULT_SETTINGS, type Settings } from './types';
export { DEFAULT_SETTINGS };

const SETTINGS_KEY = 'deepkick-settings';
const HISTORY_KEY = 'deepkick-history';

export interface HistoryEntry {
  id: string;
  timestamp: number;
  stats: {
    total: number;
    found: number;
    notFound: number;
    errors: number;
    skipped: number;
  };
  tracks: Array<{
    name: string;
    artist: string;
    title: string;
    status: 'found' | 'not_found' | 'error' | 'skipped';
  }>;
}

export async function loadSettings(): Promise<Settings> {
  try {
    const stored = localStorage.getItem(SETTINGS_KEY);
    if (stored) {
      return { ...DEFAULT_SETTINGS, ...JSON.parse(stored) };
    }
  } catch (e) {
    console.error('Failed to load settings:', e);
  }
  return { ...DEFAULT_SETTINGS };
}

export async function saveSettings(settings: Settings): Promise<void> {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch (e) {
    console.error('Failed to save settings:', e);
  }
}

export async function loadHistory(): Promise<HistoryEntry[]> {
  try {
    const stored = localStorage.getItem(HISTORY_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load history:', e);
  }
  return [];
}

export async function saveHistory(history: HistoryEntry[]): Promise<void> {
  try {
    // Keep only last 50 entries
    const trimmed = history.slice(-50);
    localStorage.setItem(HISTORY_KEY, JSON.stringify(trimmed));
  } catch (e) {
    console.error('Failed to save history:', e);
  }
}

export async function addHistoryEntry(entry: HistoryEntry): Promise<void> {
  const history = await loadHistory();
  history.push(entry);
  await saveHistory(history);
}

export async function clearHistory(): Promise<void> {
  localStorage.removeItem(HISTORY_KEY);
}