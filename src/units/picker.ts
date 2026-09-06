// Picker unit: file/folder selection and per-file manual edit/remove actions.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { mudcrack } from "rampike";

import type { AudioFile, Metadata, QueueSpec } from "@root/types";
import { getSettings } from "@root/store";
import { nothrowAsync, toast } from "@root/utils";

// State shared with the queue unit.
export const pickerState: {
	files: AudioFile[];
	metas: Map<string, Metadata>;
} = {
	files: [],
	metas: new Map(),
};

export function pickerUnit() {
	const pickFilesBtn = document.querySelector<HTMLButtonElement>("#pick-files")!;
	const pickFoldersBtn = document.querySelector<HTMLButtonElement>("#pick-folders")!;
	const clearBtn = document.querySelector<HTMLButtonElement>("#clear-list")!;
	const startBtn = document.querySelector<HTMLButtonElement>("#start-queue")!;
	const overwriteBox =
		document.querySelector<HTMLInputElement>("#overwrite-existing")!;

	pickFilesBtn.addEventListener("click", () => void pickFiles());
	pickFoldersBtn.addEventListener("click", () => void pickFolder());
	clearBtn.addEventListener("click", clearList);
	startBtn.addEventListener("click", () => {
		if (pickerState.files.length === 0) {
			toast("Pick some files first", "err");
			return;
		}
		startProcessing(overwriteBox.checked);
	});
}

async function pickFiles() {
	const selected = await open({
		multiple: true,
		title: "Select MP3/FLAC files",
		filters: [
			{
				name: "Audio",
				extensions: ["mp3", "flac"],
			},
		],
	});
	if (!selected) return;
	const paths = Array.isArray(selected) ? selected : [selected];
	const newFiles: AudioFile[] = paths.map((p) => pathToAudioFile(p));
	addFiles(newFiles);
}

async function pickFolder() {
	const selected = await open({
		directory: true,
		multiple: false,
		title: "Select a folder containing MP3/FLAC files",
	});
	if (!selected) return;
	const dir = Array.isArray(selected) ? selected[0] : selected;
	try {
		const files = await invoke<AudioFile[]>("scan_directory", { path: dir });
		addFiles(files);
	} catch (e) {
		toast(`Failed to scan folder: ${e}`, "err");
	}
}

function pathToAudioFile(p: string): AudioFile {
	const lower = p.toLowerCase();
	const kind: AudioFile["kind"] = lower.endsWith(".mp3") ? "mp3" : "flac";
	const name = p.split(/[\\/]/).pop() ?? p;
	return { uri: p, name, kind };
}

function addFiles(files: AudioFile[]) {
	for (const f of files) {
		if (pickerState.files.some((x) => x.uri === f.uri)) continue;
		pickerState.files.push(f);
		// Lazy-load metadata in the background.
		void loadMeta(f);
	}
	rerender();
}

async function loadMeta(f: AudioFile) {
	const r = await nothrowAsync(invoke<Metadata>("read_metadata", { file: f }));
	if (r.success) pickerState.metas.set(f.uri, r.value);
	rerender();
}

function clearList() {
	pickerState.files = [];
	pickerState.metas.clear();
	rerender();
}

function rerender() {
	const list = document.querySelector<HTMLUListElement>("#pick-list")!;
	const status = document.querySelector<HTMLElement>("#pick-status")!;
	list.innerHTML = "";

	if (pickerState.files.length === 0) {
		status.textContent = "No files selected";
		return;
	}
	status.textContent = `${pickerState.files.length} file${pickerState.files.length === 1 ? "" : "s"} selected`;

	for (const file of pickerState.files) {
		const meta = pickerState.metas.get(file.uri);
		const caption =
			meta?.artist && meta?.title
				? `${meta.artist} — ${meta.title}`
				: file.name;

		const item = mudcrack({
			tagName: "li",
			className: "lineout pick-row",
			contents: [
				mudcrack({
					tagName: "div",
					className: "pick-row-main",
					contents: [
						mudcrack({ tagName: "strong", contents: caption }),
						mudcrack({ className: "hint", contents: `${file.name} · ${file.kind}` }),
					],
				}),
				mudcrack({
					tagName: "div",
					className: "pick-row-actions",
					contents: [
						mudcrack({
							tagName: "button",
							contents: meta?.has_lyrics ? "view/edit" : "edit",
							events: { click: () => openEditModal(file) },
						}),
						mudcrack({
							tagName: "button",
							className: "lineout",
							contents: "remove",
							events: { click: () => removeFile(file.uri) },
						}),
					],
				}),
			],
		});
		list.appendChild(item);
	}
}

function removeFile(uri: string) {
	pickerState.files = pickerState.files.filter((f) => f.uri !== uri);
	pickerState.metas.delete(uri);
	rerender();
}

// ---------- Edit modal ----------

interface ModalEl extends HTMLElement {
	open(): void;
	close(): void;
}

function openEditModal(file: AudioFile) {
	const modal = document.querySelector<ModalEl>("#edit-modal")!;
	const titleEl = document.querySelector<HTMLElement>("#edit-modal-title")!;
	const metaEl = document.querySelector<HTMLElement>("#edit-modal-meta")!;
	const textarea = document.querySelector<HTMLTextAreaElement>(
		"#edit-modal-textarea",
	)!;
	const saveBtn = document.querySelector<HTMLButtonElement>("#edit-modal-save")!;
	const clearBtn = document.querySelector<HTMLButtonElement>("#edit-modal-clear")!;
	const cancelBtn = document.querySelector<HTMLButtonElement>("#edit-modal-cancel")!;

	const meta = pickerState.metas.get(file.uri);
	titleEl.textContent = file.name;
	metaEl.textContent =
		meta?.artist && meta?.title
			? `${meta.artist} — ${meta.title}`
			: "(no artist/title tags)";
	textarea.value = "";
	void invoke<string | null>("read_lyrics", { file }).then((v) => {
		textarea.value = v ?? "";
	});

	const close = () => modal.close();
	const onSave = async () => {
		const text = textarea.value;
		try {
			await invoke("set_lyrics", { file, lyrics: text });
			toast("Lyrics saved");
			const newMeta: Metadata = {
				artist: meta?.artist ?? null,
				title: meta?.title ?? null,
				album: meta?.album ?? null,
				has_lyrics: text.trim().length > 0,
				lyrics_len: text.length,
			};
			pickerState.metas.set(file.uri, newMeta);
			rerender();
			close();
		} catch (e) {
			toast(`Failed to save: ${e}`, "err");
		}
	};
	const onClear = async () => {
		try {
			await invoke("clear_lyrics", { file });
			toast("Lyrics removed");
			const newMeta: Metadata = {
				artist: meta?.artist ?? null,
				title: meta?.title ?? null,
				album: meta?.album ?? null,
				has_lyrics: false,
				lyrics_len: 0,
			};
			pickerState.metas.set(file.uri, newMeta);
			rerender();
			close();
		} catch (e) {
			toast(`Failed to clear: ${e}`, "err");
		}
	};

	saveBtn.onclick = onSave;
	clearBtn.onclick = onClear;
	cancelBtn.onclick = close;

	modal.open();
}

// ---------- Start processing ----------

function startProcessing(overwrite: boolean) {
	const settings = getSettings();
	const tabs = document.querySelector("ram-tabs#tabs-main") as (HTMLElement & { tab?: string }) | null;
	if (tabs) tabs.tab = "queue";

	const spec: QueueSpec = {
		files: pickerState.files,
		overwrite,
		api_base: settings.apiBase,
	};

	window.dispatchEvent(new CustomEvent("dk:start-queue", { detail: spec }));
}
