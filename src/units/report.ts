// Report unit: shows the final aggregate after a queue finishes.
// Also fires a system notification if enabled in settings.

import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { mudcrack } from "rampike";

import type { Report } from "@root/types";
import { getSettings } from "@root/store";
import { toast } from "@root/utils";

export function reportUnit() {
	window.addEventListener("dk:report-ready", (e) => {
		const report = (e as CustomEvent<Report>).detail;
		render(report);
		void maybeNotify(report);
		// Auto-switch to the report tab.
		const tabs = document.querySelector("ram-tabs#tabs-main") as (HTMLElement & { tab?: string }) | null;
		if (tabs) tabs.tab = "report";
	});
}

function render(r: Report) {
	const empty = document.getElementById("report-empty")!;
	const body = document.getElementById("report-body")!;
	empty.classList.add("hidden");
	body.classList.remove("hidden");

	(document.getElementById("report-total") as HTMLElement).textContent = String(r.total);
	(document.getElementById("report-ok") as HTMLElement).textContent = String(r.ok);
	(document.getElementById("report-existing") as HTMLElement).textContent = String(r.existing);
	(document.getElementById("report-notfound") as HTMLElement).textContent = String(r.not_found);
	(document.getElementById("report-notag") as HTMLElement).textContent = String(r.no_tag);
	(document.getElementById("report-err") as HTMLElement).textContent = String(r.errors);
}

async function maybeNotify(r: Report) {
	const s = getSettings();
	if (!s.notifyOnComplete) return;
	let granted = await isPermissionGranted();
	if (!granted) {
		const res = await requestPermission();
		granted = res === "granted";
	}
	if (!granted) return;
	const summary = `Embedded ${r.ok}/${r.total}, not found ${r.not_found}, errors ${r.errors}`;
	sendNotification({
		title: "DeepKick Lyrics",
		body: summary,
	});
	void mudcrack; // satisfy unused if removed
	toast(summary);
}
