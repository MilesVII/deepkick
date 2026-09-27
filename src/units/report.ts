import { mudcrack } from "rampike";
import { api } from "@root/api";
import type { Item, Report, Status } from "@root/types";
import { STATUS_CAPTION, STATUS_ORDER, formatTime, plural, trackCaption } from "@root/utils";
import { confirmPopup } from "@components/popup";
import { nav, onRoute } from "./navigation";
import { statusBadge } from "./queue";

let report: Report | null = null;
let filter: Status | null = null;

export function reportUnit() {
	const clear = document.querySelector<HTMLButtonElement>("#report-clear")!;
	clear.addEventListener("click", async () => {
		if (!report) return;
		if (!await confirmPopup("Discard this report?", "discard")) return;
		await api.clearReport();
		report = null;
		render();
	});

	onRoute(tab => {
		if (tab !== "report") return;
		refresh();
	});
	api.onQueueFinished(() => refresh());
}

export async function refresh() {
	report = await api.report();
	filter = null;
	render();
}

function render() {
	const summary = document.querySelector<HTMLElement>("#report-summary")!;
	const list = document.querySelector<HTMLElement>("#report-list")!;
	summary.innerHTML = "";
	list.innerHTML = "";
	document.querySelector<HTMLButtonElement>("#report-clear")!.hidden = !report;

	if (!report) {
		summary.append(mudcrack({ className: "placeholder", contents: "No report yet. Finish a queue run and it will show up here." }));
		return;
	}

	const { run } = report;
	const title = run.mode === "fetch" ? "Lyrics fetched" : "Lyrics removed";
	const when = run.finishedAt ? formatTime(run.finishedAt) : formatTime(run.startedAt);
	const seconds = run.finishedAt ? Math.round((run.finishedAt - run.startedAt) / 1000) : null;

	summary.append(
		mudcrack({ tagName: "h2", contents: run.cancelled ? `${title} (stopped)` : title }),
		mudcrack({
			className: "hint",
			contents: [when, seconds !== null ? plural(seconds, "second") : "", plural(report.items.length, "file")].filter(s => s).join(" · ")
		}),
		mudcrack({
			className: "row-wrap",
			contents: STATUS_ORDER
				.filter(status => (report!.summary[status] ?? 0) > 0)
				.map(status => mudcrack({
					tagName: "button",
					className: "strip fit pointer chip",
					attributes: {
						"data-status": status,
						"data-selected": filter === status ? "true" : "false"
					},
					contents: `${STATUS_CAPTION[status]} ${report!.summary[status]}`,
					events: {
						click: () => {
							filter = filter === status ? null : status;
							render();
						}
					}
				}))
		})
	);

	const items = filter ? report.items.filter(i => i.status === filter) : report.items;
	list.append(...items.map(itemView));
}

function itemView(item: Item) {
	const track = trackCaption(item.artist, item.title);
	return mudcrack({
		className: "lineout queue-item pointer",
		attributes: { "data-status": item.status },
		events: { click: () => nav("editor", encodeURIComponent(item.uri)) },
		contents: [
			mudcrack({
				className: "list queue-item-main",
				contents: [
					mudcrack({ className: "queue-item-name break-word", contents: item.name }),
					mudcrack({ className: "hint break-word", contents: [item.folder, track].filter(s => s).join(" · ") }),
					...(item.detail ? [mudcrack({ className: "hint break-word queue-item-detail", contents: item.detail })] : [])
				]
			}),
			mudcrack({
				className: "list queue-item-side",
				contents: [statusBadge(item)]
			})
		]
	});
}
