import { mudcrack } from "rampike";
import { RampikeModal } from "./modal";

type ButtonControl = [caption: string, cb: null | (() => void)];

export function popup(message: (container: HTMLElement) => void, buttons: ButtonControl[]): void;
export function popup(message: string,                           buttons: ButtonControl[]): void;

export function popup(
	message: string | ((container: HTMLElement) => void),
	buttons: ButtonControl[]
) {
	const popup = document.querySelector<RampikeModal>("#popup")!;
	const container = document.querySelector<HTMLElement>("#popup-message")!;
	const controls  = document.querySelector<HTMLElement>("#popup-buttons")!;

	if (typeof message === "string")
		container.textContent = message;
	else {
		container.innerHTML = "";
		message(container);
	}

	controls.innerHTML = "";
	controls.append(
		...buttons.map(([contents, cb]) =>
			mudcrack({
				tagName: "button",
				className: "lineout",
				contents,
				events: {
					click: () => {
						popup.close();
						cb?.();
					}
				}
			})
		)
	);

	popup.open();
}

/** Yes/no confirmation; resolves to `true` when the confirming button was pressed. */
export function confirmPopup(message: string, confirmCaption = "yes", cancelCaption = "cancel") {
	return new Promise<boolean>(resolve => {
		popup(message, [
			[cancelCaption, () => resolve(false)],
			[confirmCaption, () => resolve(true)]
		]);
	});
}
