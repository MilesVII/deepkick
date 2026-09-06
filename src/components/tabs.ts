// Local implementation of a tabs web component. Modeled on the one in
// references/aegir/src/components/tabs.ts but kept tiny — we only need a
// simple "one tab visible at a time" container with a tab-bar of buttons.

import { mudcrack } from "rampike";

const TAB_BTN = "ram-tab-button";
const TAB = "ram-tab";

class RampikeTab extends HTMLElement {
	selected = false;
	connectedCallback() {
		this.update();
	}
	update() {
		this.style.display = this.selected ? "" : "none";
	}
}
class RampikeTabButton extends HTMLElement {
	selected = false;
	private _container: RampikeTabContainer | null = null;

	connectedCallback() {
		const name = this.getAttribute("data-tab");
		if (!name) return;
		this._container = this.closest("ram-tabs") as RampikeTabContainer | null;
		this.addEventListener("click", () => {
			this._container?.select(name);
		});
		this.update();
	}
	update() {
		this.setAttribute("aria-selected", String(this.selected));
		this.style.fontWeight = this.selected ? "600" : "";
		this.style.borderBottom = this.selected
			? "2px solid currentColor"
			: "2px solid transparent";
	}
}
class RampikeTabContainer extends HTMLElement {
	tab = "";
	connectedCallback() {
		const bar = mudcrack({ tagName: "div", className: "row" });
		const buttons = Array.from(
			this.querySelectorAll<HTMLElement>(`${TAB_BTN}`),
		);
		buttons.forEach((b) => bar.appendChild(b));
		// Insert bar at the top.
		this.insertBefore(bar, this.firstChild);
		this.select(this.tab || buttons[0]?.getAttribute("data-tab") || "");
	}
	select(name: string) {
		this.tab = name;
		this.querySelectorAll<HTMLElement>(`${TAB_BTN}`).forEach((b) => {
			const sel = b.getAttribute("data-tab") === name;
			(b as RampikeTabButton).selected = sel;
			(b as RampikeTabButton).update?.();
		});
		this.querySelectorAll<HTMLElement>(`${TAB}`).forEach((t) => {
			const sel = t.getAttribute("data-tab") === name;
			(t as RampikeTab).selected = sel;
			(t as RampikeTab).update?.();
		});
	}
}

export function defineTabs() {
	if (!customElements.get("ram-tabs")) {
		customElements.define("ram-tabs", RampikeTabContainer);
	}
	if (!customElements.get("ram-tab")) {
		customElements.define("ram-tab", RampikeTab);
	}
	if (!customElements.get("ram-tab-button")) {
		customElements.define("ram-tab-button", RampikeTabButton);
	}
}
