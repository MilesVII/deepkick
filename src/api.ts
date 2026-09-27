import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Candidate, FileInfo, Insets, Mode, QueueState, Report, RunOptions } from "./types";

export const api = {
	queueState: () => invoke<QueueState>("queue_state"),
	addFiles: () => invoke<QueueState>("queue_add_files"),
	addDirectory: () => invoke<QueueState>("queue_add_directory"),
	remove: (ids: number[]) => invoke<QueueState>("queue_remove", { ids }),
	clear: () => invoke<QueueState>("queue_clear"),
	start: (mode: Mode, options: RunOptions) => invoke<void>("queue_start", { mode, options }),
	stop: () => invoke<void>("queue_stop"),

	report: () => invoke<Report | null>("report_get"),
	clearReport: () => invoke<void>("report_clear"),

	fileInfo: (uri: string) => invoke<FileInfo>("file_info", { uri }),
	setLyrics: (uri: string, lyrics: string | null) => invoke<void>("file_set_lyrics", { uri, lyrics }),
	search: (artist: string, title: string, album: string | null, duration: number | null) =>
		invoke<Candidate[]>("lyrics_search", { artist, title, album, duration }),

	launchAction: () => invoke<string | null>("launch_action"),
	log: (level: "info" | "warn" | "error", message: string) =>
		invoke<void>("log_message", { level, message }).catch(() => {}),
	insets: () => invoke<Insets>("insets"),

	onQueueUpdate: (cb: (state: QueueState) => void) =>
		listen<QueueState>("queue-update", e => cb(e.payload)),
	onQueueFinished: (cb: () => void) =>
		listen("queue-finished", () => cb())
};

export function errorText(error: unknown) {
	if (typeof error === "string") return error;
	if (error instanceof Error) return error.message;
	return String(error);
}
