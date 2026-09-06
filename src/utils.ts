// Shared helpers, ported/adapted from references/aegir/src/utils.ts.

import { mudcrack } from "rampike";
import type { Settings } from "./types";

export type Result<T, E = unknown> =
	| { success: true; value: T }
	| { success: false; error: E };

export function nothrow<T>(cb: () => T): Result<T, unknown> {
	try {
		return { success: true, value: cb() };
	} catch (error) {
		return { success: false, error };
	}
}

export function nothrowAsync<T>(cb: Promise<T>): Promise<Result<T, unknown>> {
	return new Promise((resolve) => {
		cb.then((value) => resolve({ success: true, value }))
			.catch((error) => resolve({ success: false, error }));
	});
}

export function sleep(ms: number) {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

export function setSelectOptions(
	target: HTMLSelectElement,
	options: [id: string, caption: string][],
	pick: string | null = null,
) {
	const optionsList = options.map(([id, caption]) =>
		mudcrack({
			tagName: "option",
			attributes: { value: id },
			contents: caption,
		}),
	);
	target.innerHTML = "";
	target.append(...optionsList);
	if (pick && options.length > 0) {
		target.value = pick;
	}
}

export function setSelectMenu(
	target: HTMLSelectElement,
	displayCaption: string,
	options: [caption: string, cb: () => void][],
) {
	const option = (id: string, caption: string) =>
		mudcrack({
			tagName: "option",
			attributes: { value: id },
			contents: caption,
		});
	const optionsList = options.map(([caption], ix) => option(String(ix), caption));
	const defaultOption = option("", displayCaption);

	target.innerHTML = "";
	target.append(defaultOption, ...optionsList);
	target.value = "";
	target.addEventListener("change", () => {
		if (!target.value) return;
		options.find((_, ix) => String(ix) === target.value)?.[1]();
		target.value = "";
	});
}

export function humanBytes(v: number | null | undefined): string {
	if (v == null) return "";
	const u = ["B", "KB", "MB", "GB"];
	let i = 0;
	let n = v;
	while (n >= 1024 && i < u.length - 1) {
		n /= 1024;
		i++;
	}
	return `${n < 10 ? n.toFixed(1) : Math.round(n)} ${u[i]}`;
}

export function debounce<F extends (...args: any[]) => void>(fn: F, ms: number): F {
	let t: ReturnType<typeof setTimeout> | null = null;
	return ((...args: any[]) => {
		if (t) clearTimeout(t);
		t = setTimeout(() => fn(...args), ms);
	}) as F;
}

// ---------- Settings persistence (localStorage) ----------

const SETTINGS_KEY = "deepkick.settings.v1";

const DEFAULT_SETTINGS: Settings = {
	apiBase: "https://lrclib.net",
	notifyOnComplete: true,
};

export function loadSettings(): Settings {
	const parsed = nothrow(() => JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}"));
	if (!parsed.success || typeof parsed.value !== "object" || parsed.value === null) {
		return { ...DEFAULT_SETTINGS };
	}
	const v = parsed.value as Partial<Settings>;
	return {
		apiBase: typeof v.apiBase === "string" && v.apiBase.length > 0
			? v.apiBase
			: DEFAULT_SETTINGS.apiBase,
		notifyOnComplete: typeof v.notifyOnComplete === "boolean"
			? v.notifyOnComplete
			: DEFAULT_SETTINGS.notifyOnComplete,
	};
}

export function saveSettings(s: Settings) {
	localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
}

// ---------- Toasts ----------

const TOAST_HOST_ID = "toast-host";

export function toast(message: string, kind: "info" | "err" = "info", ms = 4000) {
	const host = document.getElementById(TOAST_HOST_ID);
	if (!host) return;
	const el = mudcrack({
		className: `toast ${kind}`,
		contents: message,
	});
	host.appendChild(el);
	setTimeout(() => {
		el.classList.add("fading");
		setTimeout(() => el.remove(), 400);
	}, ms);
}
