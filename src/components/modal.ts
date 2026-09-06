// Modal custom element. Backed by a native <dialog>.

import { mudcrack } from "rampike";

class RampikeModal extends HTMLElement {
	private dialog!: HTMLDialogElement;
	connectedCallback() {
		if (this.dialog) return;
		this.style.display = "contents";
		const contents = Array.from(this.childNodes);
		const form = mudcrack({
			tagName: "form",
			attributes: { method: "dialog" },
			events: { submit: (e: Event) => e.preventDefault() },
		});
		this.dialog = mudcrack({
			tagName: "dialog",
			events: {
				click: (e: Event, el: HTMLDialogElement) => {
					if (e.target === el) el.close();
				},
			},
			contents: [form],
		}) as HTMLDialogElement;
		this.appendChild(this.dialog);
		contents.forEach((c) => form.appendChild(c));
	}
	open() {
		if (this.dialog.open) return;
		this.dialog.showModal();
	}
	close() {
		if (!this.dialog.open) return;
		this.dialog.close();
	}
}

export function defineModal(name: string) {
	if (!customElements.get(name)) customElements.define(name, RampikeModal);
}
