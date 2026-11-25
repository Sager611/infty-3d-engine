// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Vanilla event adapters, not Qwik.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { type ControlsEvent, listenControls } from "./controls.ts";
import type { Selection } from "./query.ts";
import { deleteShortcut, listenSceneActions } from "./scene-actions.ts";
import { DEFAULT_SNAPSHOT } from "./state.ts";

const installElement = (
	name: string,
	type: typeof EventTarget,
): (() => void) => {
	const previous = Object.getOwnPropertyDescriptor(globalThis, name);
	Object.defineProperty(globalThis, name, { value: type, configurable: true });
	return (): void => {
		if (previous) {
			Object.defineProperty(globalThis, name, previous);
		} else {
			Reflect.deleteProperty(globalThis, name);
		}
	};
};

test("delegated property input follows input and material select targets and cleans up", () => {
	class Input extends EventTarget {
		dataset = { controlKey: "parameter" };
		value = "1.8";
	}
	class Select extends EventTarget {
		dataset = { controlKey: "material" };
		value = "1";
	}
	// biome-ignore lint/security/noSecrets: Native DOM class name.
	const restoreInput = installElement("HTMLInputElement", Input);
	// biome-ignore lint/security/noSecrets: Native DOM class name.
	const restoreSelect = installElement("HTMLSelectElement", Select);
	const properties = new EventTarget();
	const input = new Input();
	const events: ControlsEvent[] = [];
	let selection: Selection = { ids: [2], active: 2 };
	const dispose = listenControls(
		{
			objectSelect: Object.assign(new EventTarget(), { value: "2" }),
			pickAll: new EventTarget(),
			properties,
		},
		() => ({ objects: DEFAULT_SNAPSHOT.objects, selection, pickAll: false }),
		(event) => {
			events.push(event);
		},
	);
	const emit = (target: EventTarget = input): void => {
		const event = new Event("input", { bubbles: true });
		Object.defineProperty(event, "target", { value: target });
		properties.dispatchEvent(event);
	};
	try {
		emit();
		expect(events).toEqual([
			{ kind: "property", id: 2, key: "parameter", value: 1.8 },
		]);
		properties.dispatchEvent(new Event("input"));
		emit(new Select());
		expect(events[1]).toEqual({
			kind: "property",
			id: 2,
			key: "material",
			value: 1,
		});
		input.dataset.controlKey = "unknown";
		emit();
		input.dataset.controlKey = "parameter";
		selection = { ids: [], active: null };
		emit();
		expect(events).toHaveLength(2);
		dispose();
		selection = { ids: [2], active: 2 };
		emit();
		expect(events).toHaveLength(2);
	} finally {
		dispose();
		restoreInput();
		restoreSelect();
	}
});

class PositionInput extends EventTarget {
	dataset = { controlKey: "y" };
	value = "1.04";
	type = "number";
	up = 0;
	down = 0;
	stepUp(): void {
		this.up += 1;
		this.value = "1.041";
	}
	stepDown(): void {
		this.down += 1;
		this.value = "1.04";
	}
}
type WheelHarness = Readonly<{
	input: PositionInput;
	events: ControlsEvent[];
	wheel: (deltaY: number, ctrlKey?: boolean) => boolean;
	dispose: () => void;
	close: () => void;
}>;
const positionWheelHarness = (): WheelHarness => {
	// biome-ignore lint/security/noSecrets: Native DOM class name.
	const restore = installElement("HTMLInputElement", PositionInput);
	const properties = new EventTarget();
	const input = new PositionInput();
	const events: ControlsEvent[] = [];
	const dispose = listenControls(
		{
			objectSelect: Object.assign(new EventTarget(), { value: "2" }),
			pickAll: new EventTarget(),
			properties,
		},
		() => ({
			objects: DEFAULT_SNAPSHOT.objects,
			selection: { ids: [2], active: 2 },
			pickAll: false,
		}),
		(event) => {
			events.push(event);
		},
	);
	const wheel = (deltaY: number, ctrlKey?: boolean): boolean => {
		const event = new Event("wheel", { cancelable: true });
		Object.defineProperties(event, {
			target: { value: input },
			deltaY: { value: deltaY },
			ctrlKey: { value: ctrlKey },
		});
		properties.dispatchEvent(event);
		return event.defaultPrevented;
	};
	return {
		input,
		events,
		wheel,
		dispose,
		close: () => {
			dispose();
			restore();
		},
	};
};
const WHEEL_DELTA = 100;
test("position wheels nudge one fine step and dispose", () => {
	const h = positionWheelHarness();
	try {
		expect(h.wheel(-WHEEL_DELTA)).toBe(true);
		expect(h.events[0]).toEqual({
			kind: "property",
			id: 2,
			key: "y",
			value: 1.041,
		});
		expect(h.wheel(WHEEL_DELTA)).toBe(true);
		expect(h.events[1]).toEqual({
			kind: "property",
			id: 2,
			key: "y",
			value: 1.04,
		});
		h.dispose();
		expect(h.wheel(-WHEEL_DELTA)).toBe(false);
		expect(h.input.up).toBe(1);
		expect(h.input.down).toBe(1);
		expect(h.events).toHaveLength(2);
	} finally {
		h.close();
	}
});
test("position wheels ignore zero, invalid, modified and nonposition input", () => {
	const h = positionWheelHarness();
	try {
		expect(h.wheel(0)).toBe(false);
		expect(h.wheel(Number.NaN)).toBe(false);
		expect(h.wheel(-WHEEL_DELTA, true)).toBe(false);
		h.input.dataset.controlKey = "size";
		expect(h.wheel(-WHEEL_DELTA)).toBe(false);
		h.input.dataset.controlKey = "y";
		h.input.value = "";
		expect(h.wheel(-WHEEL_DELTA)).toBe(false);
		expect(h.events).toHaveLength(0);
	} finally {
		h.close();
	}
});

test("property forms cannot navigate away on submit", () => {
	const properties = new EventTarget();
	const dispose = listenControls(
		{
			objectSelect: Object.assign(new EventTarget(), { value: "" }),
			pickAll: new EventTarget(),
			properties,
		},
		() => ({
			objects: DEFAULT_SNAPSHOT.objects,
			selection: { ids: [], active: null },
			pickAll: false,
		}),
		() => {
			throw new Error("unexpected control event");
		},
	);
	const event = new Event("submit", { cancelable: true });
	properties.dispatchEvent(event);
	expect(event.defaultPrevented).toBe(true);
	dispose();
	const after = new Event("submit", { cancelable: true });
	properties.dispatchEvent(after);
	expect(after.defaultPrevented).toBe(false);
});

class Button extends EventTarget {
	dataset = { sceneAction: "sphere" };
	disabled = false;
	closest(): Button {
		return this;
	}
}

test("scene actions delegate buttons, guard typing and menus, and dispose", () => {
	const restoreElement = installElement("Element", Button);
	const root = new EventTarget();
	const keyboard = new EventTarget();
	const button = new Button();
	const actions: string[] = [];
	let menuOpen = false;
	const dispose = listenSceneActions(
		root,
		keyboard,
		() => menuOpen,
		(action) => actions.push(action),
	);
	const click = (action: string): void => {
		button.dataset.sceneAction = action;
		const e = new Event("click");
		Object.defineProperty(e, "target", { value: button });
		root.dispatchEvent(e);
	};
	const key = (target: EventTarget | null): boolean => {
		const e = new Event("keydown", { cancelable: true });
		Object.defineProperties(e, {
			key: { value: "Delete" },
			target: { value: target },
		});
		keyboard.dispatchEvent(e);
		return e.defaultPrevented;
	};
	try {
		click("sphere");
		click("delete");
		click("duplicate");
		button.disabled = true;
		click("duplicate");
		button.disabled = false;
		click("unknown");
		expect(actions).toEqual(["sphere", "delete", "duplicate"]);
		expect(key(null)).toBe(true);
		menuOpen = true;
		expect(key(null)).toBe(false);
		menuOpen = false;
		for (const tagName of ["INPUT", "TEXTAREA", "SELECT", "BUTTON"]) {
			const target = Object.assign(new EventTarget(), { tagName });
			expect(key(target)).toBe(false);
		}
		expect(deleteShortcut("Backspace", null, false, false)).toBe(true);
		expect(deleteShortcut("Delete", null, true, false)).toBe(false);
		dispose();
		click("undo");
		expect(key(null)).toBe(false);
		expect(actions).toEqual(["sphere", "delete", "duplicate", "delete"]);
	} finally {
		dispose();
		restoreElement();
	}
});
