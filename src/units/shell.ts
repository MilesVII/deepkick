import { api, errorText } from "@root/api";
import { queue } from "@root/store";
import { handleSystemBack, nav } from "./navigation";
import { toast } from "./toasts";

declare global {
	interface Window {
		__deepkickInsets?: (top: number, bottom: number, left: number, right: number) => void;
		__deepkickLaunch?: () => void;
		__deepkickBack?: () => boolean;
	}
}

/**
 * Glue between the Android shell and the page: system insets, launch actions coming
 * from notifications, and keeping the queue snapshot fresh.
 */
export async function shellUnit() {
	// surface page errors in the native log, the WebView console is not visible on a device
	window.addEventListener("error", e => api.log("error", `${e.message} @ ${e.filename}:${e.lineno}`));
	window.addEventListener("unhandledrejection", e => api.log("error", `unhandled rejection: ${errorText(e.reason)}`));

	window.__deepkickInsets = applyInsets;
	window.__deepkickLaunch = handleLaunchAction;
	window.__deepkickBack = handleSystemBack;

	api.insets()
		.then(i => applyInsets(i.top, i.bottom, i.left, i.right))
		.catch(() => {});

	await api.onQueueUpdate(state => queue.set(state));
	await api.onQueueFinished(() => {
		toast("Queue finished", {
			actions: [["show report", close => { close(); nav("report"); }]]
		});
	});

	document.addEventListener("visibilitychange", () => {
		if (document.visibilityState === "visible") refreshQueue();
	});

	await refreshQueue();
	await handleLaunchAction();
}

async function refreshQueue() {
	try {
		queue.set(await api.queueState());
	} catch (error) {
		toast(`Cannot read the queue: ${errorText(error)}`);
	}
}

async function handleLaunchAction() {
	const action = await api.launchAction().catch(() => null);
	if (action === "report") nav("report");
	else if (action === "queue") nav("queue");
}

function applyInsets(top: number, bottom: number, left: number, right: number) {
	const root = document.documentElement.style;
	root.setProperty("--inset-top", `${Math.max(0, top)}px`);
	root.setProperty("--inset-bottom", `${Math.max(0, bottom)}px`);
	root.setProperty("--inset-left", `${Math.max(0, left)}px`);
	root.setProperty("--inset-right", `${Math.max(0, right)}px`);
}
