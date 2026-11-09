import { lookCamera, type Movement, STOPPED } from "./camera.ts";
import type { PureCamera } from "./state.ts";

const LOOK_STEP = 10;
const SPEED_STEP = 0.08;
const MOVEMENT_KEYS: readonly string[] = Object.freeze(["w", "a", "s", "d"]);
type KeyBindings = Readonly<{
	keyboard: EventTarget;
	visibility: EventTarget;
	hidden: () => boolean;
	blocked: () => boolean;
	getCamera: () => PureCamera;
	setCamera: (camera: PureCamera) => void;
	movement: (movement: Movement) => void;
	speed: (amount: number) => void;
	reset: () => void;
	frame: () => void;
	onError: (error: unknown) => void;
}>;
type KeyController = Readonly<{ stop(): void; dispose(): void }>;
interface KeyState {
	held: Set<string>;
	closed: boolean;
}

type EditableTarget = EventTarget &
	Readonly<{ tagName?: string; isContentEditable?: boolean }>;
function editableTarget(target: EditableTarget | null): boolean {
	if (target === null || typeof target !== "object") {
		return false;
	}
	let tag = "";
	if ("tagName" in target && typeof target.tagName === "string") {
		tag = target.tagName.toUpperCase();
	}
	return (
		["INPUT", "TEXTAREA", "SELECT", "BUTTON", "OPTION"].includes(tag) ||
		target.isContentEditable === true
	);
}
function movementFor(held: ReadonlySet<string>): Movement {
	return Object.freeze({
		forward: Number(held.has("w")) - Number(held.has("s")),
		side: Number(held.has("d")) - Number(held.has("a")),
		onlyForward: held.size === 1 && held.has("w"),
	});
}
function stopKeys(bindings: KeyBindings, state: KeyState): void {
	if (state.held.size === 0) {
		return;
	}
	state.held.clear();
	bindings.movement(STOPPED);
}
function shortcut(bindings: KeyBindings, key: string): boolean {
	switch (key) {
		case "arrowleft":
			bindings.setCamera(lookCamera(bindings.getCamera(), -LOOK_STEP, 0));
			break;
		case "arrowright":
			bindings.setCamera(lookCamera(bindings.getCamera(), LOOK_STEP, 0));
			break;
		case "arrowup":
			bindings.setCamera(lookCamera(bindings.getCamera(), 0, -LOOK_STEP));
			break;
		case "arrowdown":
			bindings.setCamera(lookCamera(bindings.getCamera(), 0, LOOK_STEP));
			break;
		case "+":
		case "=":
			bindings.speed(-SPEED_STEP);
			break;
		case "-":
		case "_":
			bindings.speed(SPEED_STEP);
			break;
		case "r":
			bindings.reset();
			break;
		case "f":
			bindings.frame();
			break;
		default:
			return false;
	}
	return true;
}
function modifiedInput(event: Event): boolean {
	return (
		("ctrlKey" in event && event.ctrlKey === true) ||
		("metaKey" in event && event.metaKey === true) ||
		("altKey" in event && event.altKey === true) ||
		("isComposing" in event && event.isComposing === true)
	);
}
function ignored(bindings: KeyBindings, event: Event): boolean {
	return (
		event.defaultPrevented ||
		modifiedInput(event) ||
		bindings.hidden() ||
		bindings.blocked() ||
		editableTarget(event.target)
	);
}
function keydown(bindings: KeyBindings, state: KeyState, event: Event): void {
	if (state.closed || !("key" in event) || typeof event.key !== "string") {
		return;
	}
	if (ignored(bindings, event)) {
		stopKeys(bindings, state);
		return;
	}
	const key = event.key.toLowerCase();
	const repeat = "repeat" in event && event.repeat === true;
	try {
		if (MOVEMENT_KEYS.includes(key)) {
			if (repeat) {
				event.preventDefault();
				return;
			}
			if (!state.held.has(key)) {
				state.held.add(key);
				bindings.movement(movementFor(state.held));
			}
			event.preventDefault();
			return;
		}
		if (key === "r" || key === "f") {
			if (repeat) {
				return;
			}
			stopKeys(bindings, state);
		}
		if (shortcut(bindings, key)) {
			event.preventDefault();
		}
	} catch (error) {
		stopKeys(bindings, state);
		bindings.onError(error);
	}
}
function listenKeys(bindings: KeyBindings): KeyController {
	const state: KeyState = { held: new Set(), closed: false };
	function stop(): void {
		stopKeys(bindings, state);
	}
	function down(event: Event): void {
		keydown(bindings, state, event);
	}
	function up(event: Event): void {
		if (
			"key" in event &&
			typeof event.key === "string" &&
			state.held.delete(event.key.toLowerCase())
		) {
			bindings.movement(movementFor(state.held));
		}
	}
	function visibility(): void {
		if (bindings.hidden()) {
			stop();
		}
	}
	function focus(event: Event): void {
		if (editableTarget(event.target) || bindings.blocked()) {
			stop();
		}
	}
	bindings.keyboard.addEventListener("keydown", down);
	bindings.keyboard.addEventListener("keyup", up);
	bindings.keyboard.addEventListener("blur", stop);
	bindings.keyboard.addEventListener("focusin", focus);
	bindings.visibility.addEventListener("visibilitychange", visibility);
	function dispose(): void {
		if (state.closed) {
			return;
		}
		stop();
		state.closed = true;
		bindings.keyboard.removeEventListener("keydown", down);
		bindings.keyboard.removeEventListener("keyup", up);
		bindings.keyboard.removeEventListener("blur", stop);
		bindings.keyboard.removeEventListener("focusin", focus);
		bindings.visibility.removeEventListener("visibilitychange", visibility);
	}
	return Object.freeze({ stop, dispose });
}
export {
	editableTarget,
	modifiedInput,
	listenKeys,
	type KeyBindings,
	type KeyController,
};
