import type { RunOptions } from "./types";
import { nothrow } from "./utils";

const STORAGE_KEY = "settings";

export type Settings = RunOptions & {
	/** Wrap long lines in the lyrics editor instead of scrolling sideways */
	wrapLyrics: boolean
};

const DEFAULT_SETTINGS: Settings = {
	overwrite: false,
	preferSynced: false,
	useFallback: true,
	wrapLyrics: false
};

type Listener = (settings: Settings) => void;
const listeners: Listener[] = [];

export function onSettingsChange(listener: Listener) {
	listeners.push(listener);
}

export function loadSettings(): Settings {
	const raw = window.localStorage.getItem(STORAGE_KEY);
	if (!raw) return { ...DEFAULT_SETTINGS };
	const parsed = nothrow(() => JSON.parse(raw));
	if (!parsed.success) return { ...DEFAULT_SETTINGS };
	return { ...DEFAULT_SETTINGS, ...parsed.value };
}

export function saveSettings(settings: Settings) {
	window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
	listeners.forEach(l => l(settings));
}
