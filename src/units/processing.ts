// Processing unit: live queue with progress bar, per-file rows, abort.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mudcrack } from "rampike";

import type { PerFileStatus, ProgressEvent, QueueSpec, Report } from "@root/types";
import { pickerState } from "@units/picker";
import { getSettings } from "@root/store";
import { toast } from "@root/utils";

export function processingUnit() {
	const abortBtn = document.querySelector<HTMLButtonElement>("#abort-queue")!;
	abortBtn.addEventListener("click", () => {
		void invoke("cancel_queue");
		toast("Aborting...");
	});

	window.addEventListener("dk:start-queue", (e) => {
		const spec = (e as CustomEvent<QueueSpec>).detail;
		void runQueue(spec);
	});

	// Subscribe to progress events.
	void listen<ProgressEvent>("processor://event", (msg) => {
		handleEvent(msg.payload);
	});
}

interface Row {
	index: number;
	name: string;
	status: PerFileStatus;
	preview: string;
	rowEl?: HTMLElement;
}

const state: {
	rows: Row[];
	total: number;
	running: boolean;
} = {
	rows: [],
	total: 0,
	running: false,
};

async function runQueue(spec: QueueSpec) {
	if (state.running) {
		toast("Already running", "err");
		return;
	}
	// Reset UI
	state.rows = spec.files.map((f, i) => ({
		index: i,
		name: f.name,
		status: "queued",
		preview: "",
	}));
	state.total = spec.files.length;
	state.running = true;
	render();

	try {
		const settings = getSettings();
		void settings;
		// Backend also uses settings.apiBase from spec; we pass it explicitly.
		await invoke<Report>("process_queue", { spec });
	} catch (e) {
		toast(`Queue failed: ${e}`, "err");
	} finally {
		state.running = false;
	}
}

function handleEvent(ev: ProgressEvent) {
	switch (ev.kind) {
		case "started":
			state.total = ev.total;
			render();
			break;
		case "file":
			if (state.rows[ev.index]) {
				state.rows[ev.index].status = ev.status;
				state.rows[ev.index].name = ev.name;
				state.rows[ev.index].preview = ev.preview ?? "";
				updateRow(state.rows[ev.index]);
				updateProgress();
			}
			break;
		case "aborted":
			toast("Aborted");
			state.running = false;
			break;
		case "done":
			state.running = false;
			updateProgress();
			// Notify the report unit by re-dispatching.
			window.dispatchEvent(
				new CustomEvent("dk:report-ready", { detail: ev.report }),
			);
			break;
	}
}

function updateProgress() {
	const done = state.rows.filter((r) => r.status !== "queued").length;
	const total = state.total || state.rows.length;
	const bar = document.querySelector<HTMLProgressElement>("#queue-progress")!;
	const txt = document.querySelector<HTMLElement>("#queue-progress-text")!;
	bar.value = done;
	bar.max = total;
	txt.textContent = `${done} / ${total}`;
}

function render() {
	const list = document.querySelector<HTMLUListElement>("#queue-list")!;
	list.innerHTML = "";
	for (const row of state.rows) {
		const li = buildRow(row);
		row.rowEl = li;
		list.appendChild(li);
	}
	updateProgress();
}

function updateRow(row: Row) {
	if (!row.rowEl) return;
	const statusEl = row.rowEl.querySelector<HTMLElement>(".row-status");
	if (statusEl) statusEl.textContent = statusLabel(row.status) + (row.preview ? " · preview below" : "");
	const previewEl = row.rowEl.querySelector<HTMLElement>(".row-preview");
	if (previewEl) previewEl.textContent = row.preview || "";
	row.rowEl.dataset.status = row.status;
}

function buildRow(row: Row): HTMLElement {
	const li = mudcrack({
		tagName: "li",
		className: "lineout queue-row",
		attributes: { "data-status": row.status },
		contents: [
			mudcrack({ tagName: "strong", contents: row.name }),
			mudcrack({ className: "row-status hint", contents: statusLabel(row.status) }),
			mudcrack({ className: "row-preview hint" }),
		],
	}) as HTMLElement;
	return li;
}

function statusLabel(s: PerFileStatus): string {
	switch (s) {
		case "queued":
			return "queued…";
		case "ok":
			return "✓ embedded";
		case "not_found":
			return "✗ not found";
		case "no_tag":
			return "✗ no artist/title";
		case "existing":
			return "— already had lyrics";
		case "err":
			return "✗ error";
	}
}

// Picker imports this so we can let it access metas if needed; no-op reference.
void pickerState;
