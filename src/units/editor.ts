import { mudcrack } from "rampike";
import { api, errorText } from "@root/api";
import { queue } from "@root/store";
import { loadSettings, onSettingsChange } from "@root/settings";
import type { Candidate, FileInfo } from "@root/types";
import { formatDuration } from "@root/utils";
import { confirmPopup } from "@components/popup";
import { RampikeModal } from "@components/modal";
import { onRoute } from "./navigation";
import { toast } from "./toasts";

let currentUri: string | null = null;
let info: FileInfo | null = null;
let loadedLyrics = "";

function els() {
	return {
		name:     document.querySelector<HTMLElement>("#editor-name")!,
		folder:   document.querySelector<HTMLElement>("#editor-folder")!,
		meta:     document.querySelector<HTMLElement>("#editor-meta")!,
		artist:   document.querySelector<HTMLInputElement>("#editor-artist")!,
		title:    document.querySelector<HTMLInputElement>("#editor-title")!,
		lyrics:   document.querySelector<HTMLTextAreaElement>("#editor-lyrics")!,
		save:     document.querySelector<HTMLButtonElement>("#editor-save")!,
		search:   document.querySelector<HTMLButtonElement>("#editor-search")!,
		remove:   document.querySelector<HTMLButtonElement>("#editor-remove")!,
		modal:    document.querySelector<RampikeModal>("#search-modal")!,
		results:  document.querySelector<HTMLElement>("#search-results")!,
		close:    document.querySelector<HTMLButtonElement>("#search-close")!
	};
}

export function editorUnit() {
	const e = els();

	e.save.addEventListener("click", save);
	e.remove.addEventListener("click", remove);
	e.search.addEventListener("click", search);
	e.close.addEventListener("click", () => e.modal.close());
	e.lyrics.addEventListener("input", () => updateDirty());
	e.lyrics.addEventListener("focus", () => setTimeout(() => e.lyrics.scrollIntoView({ block: "nearest" }), 300));

	const applyWrap = () => e.lyrics.classList.toggle("wrap", loadSettings().wrapLyrics);
	onSettingsChange(applyWrap);
	applyWrap();

	onRoute((tab, param) => {
		if (tab !== "editor") return;
		const uri = param ? decodeURIComponent(param) : null;
		if (!uri) return;
		load(uri);
	});
}

function setBusy(busy: boolean) {
	const e = els();
	[e.save, e.search, e.remove].forEach(b => b.disabled = busy);
}

function updateDirty() {
	const e = els();
	const dirty = e.lyrics.value.replace(/\r\n/g, "\n").trim() !== loadedLyrics;
	e.save.classList.toggle("accent", dirty);
	e.save.textContent = dirty ? "save *" : "save";
}

async function load(uri: string) {
	const e = els();
	currentUri = uri;
	info = null;
	loadedLyrics = "";

	const item = queue.item(uri);
	e.name.textContent = item?.name ?? uri.split("/").pop() ?? uri;
	e.folder.textContent = item?.folder ?? "";
	e.meta.textContent = "reading tags…";
	e.artist.value = item?.artist ?? "";
	e.title.value = item?.title ?? "";
	e.lyrics.value = "";
	updateDirty();
	setBusy(true);

	try {
		const loaded = await api.fileInfo(uri);
		if (currentUri !== uri) return;
		info = loaded;
		e.artist.value = loaded.artist ?? "";
		e.title.value = loaded.title ?? "";
		e.lyrics.value = loaded.lyrics ?? "";
		loadedLyrics = (loaded.lyrics ?? "").trim();
		renderMeta();
	} catch (error) {
		e.meta.textContent = `cannot read the file: ${errorText(error)}`;
	} finally {
		setBusy(false);
		updateDirty();
	}
}

function renderMeta() {
	if (!info) return;
	els().meta.textContent = [
		info.kind.toUpperCase(),
		info.album ?? "",
		formatDuration(info.durationSecs),
		info.lyrics ? "lyrics embedded" : "no lyrics",
		info.repaired ? `tags repaired from ${info.repaired === "cp1251" ? "CP1251" : "UTF-8"}` : ""
	].filter(s => s).join(" · ");
}

async function save() {
	const e = els();
	if (!currentUri) return;
	const text = e.lyrics.value.replace(/\r\n/g, "\n").trim();
	setBusy(true);
	try {
		await api.setLyrics(currentUri, text || null, loadSettings().fixEncoding);
		loadedLyrics = text;
		toast(text ? "Lyrics saved" : "Lyrics removed");
		if (info) info.lyrics = text || null;
		renderMeta();
	} catch (error) {
		toast(`Cannot save: ${errorText(error)}`);
	} finally {
		setBusy(false);
		updateDirty();
	}
}

async function remove() {
	const e = els();
	if (!currentUri) return;
	if (!e.lyrics.value.trim() && !loadedLyrics) {
		toast("There are no lyrics to remove");
		return;
	}
	if (!await confirmPopup("Remove the lyrics from this file?", "remove")) return;
	e.lyrics.value = "";
	await save();
}

async function search() {
	const e = els();
	const artist = e.artist.value.trim();
	const title = e.title.value.trim();
	if (!artist || !title) {
		toast("Artist and title are needed to search");
		return;
	}
	setBusy(true);
	e.search.textContent = "searching…";
	try {
		const candidates = await api.search(artist, title, info?.album ?? null, info?.durationSecs ?? null);
		showResults(candidates);
	} catch (error) {
		toast(`Search failed: ${errorText(error)}`);
	} finally {
		setBusy(false);
		e.search.textContent = "search online";
	}
}

function showResults(candidates: Candidate[]) {
	const e = els();
	e.results.innerHTML = "";
	if (candidates.length === 0) {
		e.results.append(mudcrack({ className: "placeholder", contents: "Nothing found. Try adjusting the artist or the title." }));
	} else {
		const preferSynced = loadSettings().preferSynced;
		e.results.append(...candidates.map(c => candidateView(c, preferSynced)));
	}
	e.modal.open();
}

function candidateView(c: Candidate, preferSynced: boolean) {
	const e = els();
	const use = (text: string | null) => {
		if (!text) return;
		e.lyrics.value = text;
		updateDirty();
		e.modal.close();
		toast("Lyrics pasted, press save to write them into the file");
	};
	const durationDelta = info?.durationSecs && c.duration
		? Math.round(c.duration - info.durationSecs)
		: null;
	const buttons: Element[] = [];
	if (c.instrumental) {
		buttons.push(mudcrack({ className: "badge", attributes: { "data-status": "instrumental" }, contents: "instrumental" }));
	}
	const order = preferSynced ? ["synced", "plain"] : ["plain", "synced"];
	for (const kind of order) {
		const text = kind === "plain" ? c.plain : c.synced;
		if (!text) continue;
		buttons.push(mudcrack({
			tagName: "button",
			className: "lineout fit",
			contents: kind === "plain" ? "use plain" : "use synced",
			events: { click: () => use(text) }
		}));
	}
	return mudcrack({
		className: "lineout list",
		contents: [
			mudcrack({ className: "queue-item-name break-word", contents: `${c.artist} — ${c.title}` }),
			mudcrack({
				className: "hint break-word",
				contents: [
					c.album,
					formatDuration(c.duration) + (durationDelta !== null && durationDelta !== 0 ? ` (${durationDelta > 0 ? "+" : ""}${durationDelta} s)` : ""),
					c.source
				].filter(s => s).join(" · ")
			}),
			mudcrack({ className: "row-wrap", contents: buttons })
		]
	});
}
