import type { QueueState } from "./types";

type Listener = (state: QueueState) => void;

/**
 * Non-reactive holder of the queue snapshot coming from Rust.
 * Units subscribe and re-render themselves explicitly.
 */
class QueueStore {
	state: QueueState = { items: [], run: null };
	private listeners: Listener[] = [];

	set(state: QueueState) {
		this.state = state;
		this.listeners.forEach(l => l(state));
	}

	subscribe(listener: Listener) {
		this.listeners.push(listener);
	}

	get running() {
		return this.state.run?.active ?? false;
	}

	item(uri: string) {
		return this.state.items.find(i => i.uri === uri) ?? null;
	}
}

export const queue = new QueueStore();
