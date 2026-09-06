// Labeled input custom element. Renders a <fieldset> wrapping a <legend> and
// either an <input> (default) or whatever the first child is (e.g. a <select>).

import { mudcrack } from "rampike";

class RampikeLabeled extends HTMLElement {
	connectedCallback() {
		if (this.dataset.bound === "1") return;
		this.dataset.bound = "1";
		const legend = this.getAttribute("legend") ?? "";
		const id = this.id || `ram-labeled-${Math.random().toString(36).slice(2)}`;
		// Collect children (e.g. <input> or <select>) and forward arbitrary attrs.
		const children = Array.from(this.children);
		const attributes: Record<string, string> = { id };
		const merged = this.getAttributeNames();
		for (const name of merged) {
			if (name === "legend" || name === "id") continue;
			attributes[name] = this.getAttribute(name) ?? "";
		}
		const fieldset = mudcrack({
			tagName: "fieldset",
			contents: [
				mudcrack({
					tagName: "legend",
					attributes: { for: id },
					contents: legend,
				}),
				...children,
			],
		});
		this.replaceWith(fieldset);
		// Re-attach children to the new fieldset if replaceWith didn't move them.
		void attributes;
	}
}

export function defineFieldset(name: string) {
	if (!customElements.get(name)) customElements.define(name, RampikeLabeled);
}
