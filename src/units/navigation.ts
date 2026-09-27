import { RampikeTabs } from "@components/tabs";
import { getRoute } from "@root/utils";

const TABS = ["queue", "report", "editor", "settings"];
const HUB = "queue";
const STACK_LIMIT = 16;

type RouteListener = (tab: string, param: string) => void;
const listeners: RouteListener[] = [];

/**
 * The page keeps its own navigation stack instead of relying on WebView history:
 * a cold start from a notification lands straight on the report with no history,
 * and the queue is the hub every other screen returns to.
 */
const stack: string[] = [];
let currentHash = "";
/** Hash set by nav()/back(); anything else comes from outside (browser history). */
let expectedHash: string | null = null;

function go(target: string) {
	expectedHash = target;
	window.location.hash = target;
}

export function nav(to: string, param?: string) {
	const target = param ? `${to}.${param}` : to;
	if (target === currentHash) return;
	if (to !== HUB && currentHash) {
		stack.push(currentHash);
		if (stack.length > STACK_LIMIT) stack.shift();
	}
	go(target);
}

/** Returns to the previous screen (or the queue). */
export function back() {
	go(stack.pop() ?? HUB);
}

/** Handles the system back gesture; `false` means there is nowhere to go. */
export function handleSystemBack(): boolean {
	const openDialog = document.querySelector<HTMLDialogElement>("dialog[open]");
	if (openDialog) {
		openDialog.close();
		return true;
	}
	if (getRoute()[0] && getRoute()[0] !== HUB) {
		back();
		return true;
	}
	return false;
}

export function onRoute(listener: RouteListener) {
	listeners.push(listener);
}

export function navigationUnit() {
	const tabs = document.querySelector<RampikeTabs>("ram-tabs#tabs-main")!;

	function readHash() {
		const [route, param] = getRoute();
		const tab = TABS.includes(route) ? route : HUB;
		const hash = window.location.hash.slice(1);
		if (expectedHash !== hash && currentHash && hash !== currentHash && tab !== HUB) {
			// hash changed by other means (e.g. browser history); keep the stack consistent
			stack.push(currentHash);
		}
		expectedHash = null;
		currentHash = hash;
		if (tab === HUB) stack.length = 0;
		tabs.tab = tab;
		listeners.forEach(l => l(tab, param));
		// every tab has its own scroll container; reset it on navigation
		document.querySelectorAll<HTMLElement>("ram-tab:not([hidden]) .main").forEach(m => m.scrollTop = 0);
	}

	window.addEventListener("hashchange", readHash);
	readHash();

	const buttons = document.querySelectorAll<HTMLButtonElement>("button[data-to]");
	buttons.forEach(b => b.addEventListener("click", () => {
		if (b.dataset.to === HUB && getRoute()[0] !== HUB) back();
		else nav(b.dataset.to!);
	}));
}
