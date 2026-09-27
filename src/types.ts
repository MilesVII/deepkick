export type Status =
	| "pending"
	| "processing"
	| "done"
	| "notFound"
	| "noTag"
	| "existing"
	| "instrumental"
	| "removed"
	| "empty"
	| "unsupported"
	| "error";

export type Mode = "fetch" | "remove";

export type Item = {
	id: number,
	uri: string,
	name: string,
	folder: string,
	size: number,
	status: Status,
	artist: string | null,
	title: string | null,
	detail: string | null
};

export type Run = {
	mode: Mode,
	active: boolean,
	cancelled: boolean,
	startedAt: number,
	finishedAt: number | null,
	current: number,
	total: number
};

export type QueueState = {
	items: Item[],
	run: Run | null
};

export type Report = {
	run: Run,
	items: Item[],
	summary: Partial<Record<Status, number>>
};

export type RunOptions = {
	overwrite: boolean,
	preferSynced: boolean,
	useFallback: boolean
};

export type FileInfo = {
	kind: "mp3" | "flac",
	artist: string | null,
	title: string | null,
	album: string | null,
	durationSecs: number | null,
	lyrics: string | null
};

export type Candidate = {
	id: number,
	artist: string,
	title: string,
	album: string,
	duration: number,
	instrumental: boolean,
	plain: string | null,
	synced: string | null,
	source: string
};

export type Insets = {
	top: number,
	bottom: number,
	left: number,
	right: number
};

export type Result<T, E> =
	| { success: true, value: T }
	| { success: false, error: E };
