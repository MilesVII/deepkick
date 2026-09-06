// Shared types between Rust commands and TS units.

export type AudioKind = "mp3" | "flac";

export interface AudioFile {
	uri: string;
	name: string;
	kind: AudioKind;
	size_bytes?: number | null;
}

export interface Metadata {
	artist?: string | null;
	title?: string | null;
	album?: string | null;
	has_lyrics: boolean;
	lyrics_len?: number | null;
}

export type PerFileStatus =
	| "queued"
	| "ok"
	| "not_found"
	| "no_tag"
	| "existing"
	| "err";

export interface ReportEntry {
	uri: string;
	name: string;
	status: PerFileStatus;
	source_link?: string | null;
	error?: string | null;
}

export interface Report {
	total: number;
	ok: number;
	not_found: number;
	no_tag: number;
	existing: number;
	errors: number;
	entries: ReportEntry[];
}

export interface QueueSpec {
	files: AudioFile[];
	overwrite: boolean;
	api_base?: string | null;
}

export type ProgressEvent =
	| { kind: "started"; total: number }
	| {
			kind: "file";
			index: number;
			name: string;
			status: PerFileStatus;
			preview?: string | null;
			error?: string | null;
	  }
	| { kind: "done"; report: Report }
	| { kind: "aborted" };

export interface Settings {
	apiBase: string;
	notifyOnComplete: boolean;
}
