import type { Result, Status } from "./types";

export function nothrow<T>(cb: () => T): Result<T, any> {
	try {
		return { success: true, value: cb() };
	} catch (error: any) {
		return { success: false, error };
	}
}

export function nothrowAsync<T>(cb: Promise<T>): Promise<Result<T, any>> {
	return new Promise(resolve => {
		cb
			.then(value => resolve({ success: true, value }))
			.catch(error => resolve({ success: false, error }));
	});
}

export function getRoute() {
	const raw = window.location.hash.slice(1);
	const dot = raw.indexOf(".");
	if (dot < 0) return [raw, ""] as const;
	return [raw.slice(0, dot), raw.slice(dot + 1)] as const;
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`) {
	return `${count} ${count === 1 ? singular : pluralForm}`;
}

export function formatDuration(seconds: number | null | undefined) {
	if (!seconds || seconds <= 0) return "";
	const total = Math.round(seconds);
	const m = Math.floor(total / 60);
	const s = total % 60;
	return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatSize(bytes: number) {
	if (!bytes) return "";
	if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
	return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatTime(ms: number) {
	return new Date(ms).toLocaleString();
}

export const STATUS_CAPTION: Record<Status, string> = {
	pending: "pending",
	processing: "working",
	done: "embedded",
	notFound: "not found",
	noTag: "no tag",
	existing: "has lyrics",
	instrumental: "instrumental",
	removed: "removed",
	empty: "no lyrics",
	unsupported: "unsupported",
	error: "error"
};

export const STATUS_ORDER: Status[] = [
	"done", "removed", "existing", "notFound", "instrumental",
	"noTag", "empty", "unsupported", "error", "processing", "pending"
];

export function trackCaption(artist: string | null, title: string | null) {
	if (artist && title) return `${artist} — ${title}`;
	return artist || title || "";
}
