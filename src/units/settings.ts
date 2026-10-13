import { fromTemplateFirst } from "rampike";
import { getVersion } from "@tauri-apps/api/app";
import { loadSettings, saveSettings } from "@root/settings";

const STORAGE_KEY_THEME = "theme";

export function settingsUnit() {
	initOptions();
	initTheme();
	getVersion().then(v => {
		const el = document.querySelector<HTMLElement>("#settings-version");
		if (el) el.textContent = `v${v}`;
	}).catch(() => {});
}

function initOptions() {
	const overwrite = document.querySelector<HTMLInputElement>("#settings-overwrite")!;
	const synced    = document.querySelector<HTMLInputElement>("#settings-synced")!;
	const fallback  = document.querySelector<HTMLInputElement>("#settings-fallback")!;
	const encoding  = document.querySelector<HTMLInputElement>("#settings-encoding")!;
	const wrap      = document.querySelector<HTMLInputElement>("#settings-wrap")!;

	const settings = loadSettings();
	overwrite.checked = settings.overwrite;
	synced.checked = settings.preferSynced;
	fallback.checked = settings.useFallback;
	encoding.checked = settings.fixEncoding;
	wrap.checked = settings.wrapLyrics;

	const persist = () => saveSettings({
		overwrite: overwrite.checked,
		preferSynced: synced.checked,
		useFallback: fallback.checked,
		fixEncoding: encoding.checked,
		wrapLyrics: wrap.checked
	});
	[overwrite, synced, fallback, encoding, wrap].forEach(i => i.addEventListener("change", persist));
}

function initTheme() {
	const theme = window.localStorage.getItem(STORAGE_KEY_THEME);
	if (theme) switchTheme(theme);

	// the bundler merges every stylesheet into one file, so scan them all for theme rules
	const rules: CSSStyleRule[] = [];
	for (const ss of Array.from(document.styleSheets)) {
		try {
			rules.push(...Array.from(ss.cssRules).filter(r => r instanceof CSSStyleRule) as CSSStyleRule[]);
		} catch {
			// cross-origin stylesheet, nothing to read
		}
	}

	const themes = rules
		.map(r => r.selectorText.match(/^\.theme-(.*)$/))
		.filter((m): m is RegExpMatchArray => m !== null);

	document.querySelector("#settings-themes")?.append(...themes.map(t => selectorItem(t[1])));
}

function selectorItem(name: string) {
	const template = document.querySelector<HTMLTemplateElement>("#template-theme-selector")!;
	const button = fromTemplateFirst<HTMLButtonElement>(template)!;
	const themeClassName = `theme-${name}`;
	button.classList.add(themeClassName);
	button.title = name;
	button.addEventListener("click", () => switchTheme(themeClassName));
	return button;
}

function switchTheme(themeClassName: string) {
	// the theme goes on <html> so the canvas under the system bars is themed as well
	const root = document.documentElement;
	root.classList.forEach(c => {
		if (c.startsWith("theme-")) root.classList.remove(c);
	});
	root.classList.add(themeClassName);
	window.localStorage.setItem(STORAGE_KEY_THEME, themeClassName);
}
