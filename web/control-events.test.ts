// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Vanilla event adapters, not Qwik.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { type ControlsEvent, listenControls } from "./controls.ts";
import type { Selection } from "./query.ts";
import { DEFAULT_SNAPSHOT } from "./state.ts";

test("delegated property input follows the input target and cleans up", () => {
	class Input extends EventTarget {
		dataset = { controlKey: "parameter" };
		value = "1.8";
	}
	// biome-ignore lint/security/noSecrets: Native DOM class name, not a credential.
	const inputClass = "HTMLInputElement";
	const previous = Object.getOwnPropertyDescriptor(globalThis, inputClass);
	Object.defineProperty(globalThis, inputClass, {
		value: Input,
		configurable: true,
	});
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
	const emit = (): void => {
		const event = new Event("input", { bubbles: true });
		Object.defineProperty(event, "target", { value: input });
		properties.dispatchEvent(event);
	};
	try {
		emit();
		expect(events).toEqual([
			{ kind: "property", id: 2, key: "parameter", value: 1.8 },
		]);
		properties.dispatchEvent(new Event("input"));
		input.dataset.controlKey = "unknown";
		emit();
		input.dataset.controlKey = "parameter";
		selection = { ids: [], active: null };
		emit();
		expect(events).toHaveLength(1);
		dispose();
		selection = { ids: [2], active: 2 };
		emit();
		expect(events).toHaveLength(1);
	} finally {
		dispose();
		if (previous) {
			Object.defineProperty(globalThis, inputClass, previous);
		} else {
			Reflect.deleteProperty(globalThis, inputClass);
		}
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
