import { mudcrack } from "rampike";
import { api, errorText } from "@root/api";
import { queue } from "@root/store";
import { loadSettings } from "@root/settings";
import type { Item, Mode, QueueState } from "@root/types";
import { STATUS_CAPTION, plural, trackCaption } from "@root/utils";
import { confirmPopup } from "@components/popup";
import { toast } from "./toasts";
import { nav } from "./navigation";

export function queueUnit() {
	const addFiles  = document.querySelector<HTMLButtonElement>("#queue-add-files")!;
	const addFolder = document.querySelector<HTMLButtonElement>("#queue-add-folder")!;
	const clear     = document.querySelector<HTMLButtonElement>("#queue-clear")!;

	let picking = false;
	async function pick(action: () => Promise<QueueState>, what: string) {
		if (picking || queue.running) return;
		picking = true;
		const before = queue.state.items.length;
		try {
			const state = await action();
			queue.set(state);
			const added = state.items.length - before;
			if (added > 0) toast(`${plural(added, "file")} added`);
			else toast(`No new ${what} selected`);
		} catch (error) {
			toast(`Cannot add ${what}: ${errorText(error)}`);
		} finally {
			picking = false;
		}
	}

	addFiles.addEventListener("click", () => pick(api.addFiles, "files"));
	addFolder.addEventListener("click", () => pick(api.addDirectory, "files"));
	clear.addEventListener("click", async () => {
		if (queue.running || queue.state.items.length === 0) return;
		if (!await confirmPopup(`Remove all ${plural(queue.state.items.length, "file")} from the queue? The files themselves stay untouched.`, "clear")) return;
		queue.set(await api.clear());
	});

	queue.subscribe(render);
	render(queue.state);
}

function render(state: QueueState) {
	renderStatus(state);
	renderList(state);
	renderActions(state);

	const running = state.run?.active ?? false;
	document
		.querySelectorAll<HTMLButtonElement>("#queue-add-files, #queue-add-folder, #queue-clear")
		.forEach(b => b.disabled = running);
}

function renderStatus(state: QueueState) {
	const container = document.querySelector<HTMLElement>("#queue-status")!;
	container.innerHTML = "";

	const run = state.run;
	if (run?.active) {
		const done = state.items.filter(i => i.status !== "pending" && i.status !== "processing").length;
		const current = state.items[run.current];
		container.append(
			mudcrack({
				className: "row baseline",
				contents: [
					mudcrack({ contents: run.mode === "fetch" ? "Fetching lyrics" : "Removing lyrics" }),
					mudcrack({ className: "hint float-end", contents: `${done} / ${run.total}` })
				]
			}),
			mudcrack({
				tagName: "progress",
				className: "wide",
				attributes: { max: String(run.total), value: String(done) }
			}),
			mudcrack({ className: "hint ellipsis", contents: current ? current.name : "" })
		);
		return;
	}

	if (state.items.length === 0) {
		container.append(mudcrack({
			className: "placeholder",
			contents: "Pick MP3 or FLAC files, or a whole folder, to fill the queue"
		}));
		return;
	}

	const caption = plural(state.items.length, "file");
	container.append(mudcrack({ className: "hint", contents: caption }));
}

function renderList(state: QueueState) {
	const list = document.querySelector<HTMLElement>("#queue-list")!;
	list.innerHTML = "";
	const running = state.run?.active ?? false;
	list.append(...state.items.map(item => itemView(item, running)));
}

export function statusBadge(item: Item) {
	return mudcrack({
		className: "badge",
		attributes: { "data-status": item.status },
		contents: STATUS_CAPTION[item.status]
	});
}

function itemView(item: Item, running: boolean) {
	const open = () => nav("editor", encodeURIComponent(item.uri));
	const track = trackCaption(item.artist, item.title);
	return mudcrack({
		className: "lineout queue-item",
		attributes: { "data-status": item.status },
		contents: [
			mudcrack({
				className: "list queue-item-main pointer",
				events: { click: open },
				contents: [
					mudcrack({ className: "queue-item-name break-word", contents: item.name }),
					mudcrack({ className: "hint break-word", contents: [item.folder, track].filter(s => s).join(" · ") }),
					...(item.detail ? [mudcrack({ className: "hint break-word queue-item-detail", contents: item.detail })] : [])
				]
			}),
			mudcrack({
				className: "list queue-item-side",
				contents: [
					statusBadge(item),
					mudcrack({
						tagName: "button",
						className: "lineout fit queue-item-remove",
						contents: "✕",
						attributes: running ? { disabled: "" } : {},
						events: {
							click: async () => {
								if (queue.running) return;
								queue.set(await api.remove([item.id]));
							}
						}
					})
				]
			})
		]
	});
}

function renderActions(state: QueueState) {
	const bar = document.querySelector<HTMLElement>("#queue-actions")!;
	bar.innerHTML = "";

	if (state.run?.active) {
		let stopping = false;
		bar.append(
			mudcrack({ className: "hint grow", contents: "Keep the app installed; it works in the background" }),
			mudcrack({
				tagName: "button",
				className: "lineout fit",
				contents: "stop",
				events: {
					click: async (_, el) => {
						if (stopping) return;
						stopping = true;
						el.textContent = "stopping…";
						await api.stop();
					}
				}
			})
		);
		return;
	}

	if (state.items.length === 0) {
		bar.hidden = true;
		return;
	}
	bar.hidden = false;

	const start = async (mode: Mode) => {
		if (queue.running) return;
		if (mode === "remove") {
			const ok = await confirmPopup(`Remove embedded lyrics from ${plural(state.items.length, "file")}?`, "remove");
			if (!ok) return;
		}
		try {
			await api.start(mode, loadSettings());
		} catch (error) {
			toast(errorText(error));
		}
	};

	bar.append(
		mudcrack({
			tagName: "button",
			className: "lineout fit",
			contents: "remove lyrics",
			events: { click: () => start("remove") }
		}),
		mudcrack({
			tagName: "button",
			className: "lineout grow accent",
			contents: "fetch lyrics",
			events: { click: () => start("fetch") }
		})
	);
}
