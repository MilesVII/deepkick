// DeepKick Lyrics — entry point.
//
// Wires up the rampike web components and starts each unit on DOMContentLoaded,
// mirroring references/aegir/src/index.ts.

import { defineTabs } from "@rampike/tabs";
import { defineModal } from "@rampike/modal";
import { defineFieldset } from "@rampike/fieldset";

import { pickerUnit } from "@units/picker";
import { processingUnit } from "@units/processing";
import { reportUnit } from "@units/report";
import { settingsUnit } from "@units/settings";

import { loadSettings, saveSettings } from "@root/utils";
import type { Settings } from "@root/types";

defineTabs();
defineModal("ram-modal");
defineFieldset("ram-labeled");

// Store the active settings globally so any unit can read them.
const state = {
	settings: loadSettings(),
};

// Apply settings to inputs as soon as DOM is ready, before units mount.
function applySettingsToUI(s: Settings) {
	const apiInput = document.querySelector<HTMLInputElement>(
		"#setting-api-base input",
	);
	if (apiInput) apiInput.value = s.apiBase;
	const notifySelect = document.querySelector<HTMLSelectElement>(
		"#setting-notify-select",
	);
	if (notifySelect) notifySelect.value = String(s.notifyOnComplete);
}

const units: (() => void)[] = [
	pickerUnit,
	processingUnit,
	reportUnit,
	settingsUnit,
];

window.addEventListener("DOMContentLoaded", () => {
	applySettingsToUI(state.settings);
	units.forEach((u) => u());

	// Persist settings on change.
	const apiInput = document.querySelector<HTMLInputElement>(
		"#setting-api-base input",
	);
	apiInput?.addEventListener("change", () => {
		state.settings = { ...state.settings, apiBase: apiInput.value };
		saveSettings(state.settings);
	});
	const notifySelect = document.querySelector<HTMLSelectElement>(
		"#setting-notify-select",
	);
	notifySelect?.addEventListener("change", () => {
		state.settings = {
			...state.settings,
			notifyOnComplete: notifySelect.value === "true",
		};
		saveSettings(state.settings);
	});
});

export function getSettings(): Settings {
	return state.settings;
}
