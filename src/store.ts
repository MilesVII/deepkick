// Small app-wide state shared between units. Kept tiny on purpose.

import { loadSettings } from "./utils";
import type { Settings } from "./types";

const settings = loadSettings();

export function getSettings(): Settings {
	return settings;
}

export function setSettings(s: Settings) {
	settings.apiBase = s.apiBase;
	settings.notifyOnComplete = s.notifyOnComplete;
}
