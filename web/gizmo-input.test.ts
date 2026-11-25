// biome-ignore-all lint/style/noMagicNumbers: Pointer and pose fixtures.
// biome-ignore-all lint/nursery/useMaxParams: Small test event constructors.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Bun fixtures, not Qwik.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import {
	arcPoint,
	type Handle,
	makeProjection,
	type Pixel,
	type Projection,
	project,
} from "./gizmo.ts";
import {
	type GizmoContext,
	type GizmoInput,
	listenGizmo,
} from "./gizmo-input.ts";
import { IDENTITY_ROTATION, rotateVector } from "./rotation.ts";
import type { SceneObject, ScenePatch } from "./scene.ts";

const required = <T>(value: T | undefined): T => {
	if (value === undefined) {
		throw new Error("Missing fixture");
	}
	return value;
};
const projection: Projection = required(
	makeProjection(
		{
			yaw: 0,
			pitch: 0,
			distance: 5,
			target: [0, 0, 0],
		},
		[1000, 800],
	),
);
const original: SceneObject = {
	id: 1,
	kind: "finite",
	position: [0, 0, 0],
	size: 1,
	parameter: 0,
};
class Surface extends EventTarget {
	captured = new Set<number>();
	failCapture = false;
	getBoundingClientRect(): { left: number; top: number } {
		return { left: 10, top: 20 };
	}
	setPointerCapture(id: number): void {
		if (this.failCapture) {
			throw new Error("capture failed");
		}
		this.captured.add(id);
	}
	hasPointerCapture(id: number): boolean {
		return this.captured.has(id);
	}
	releasePointerCapture(id: number): void {
		this.captured.delete(id);
	}
}
const dispatch = (
	target: EventTarget,
	type: string,
	fields: Readonly<Record<string, unknown>>,
): boolean => {
	const event = new Event(type, { cancelable: true });
	Object.defineProperties(
		event,
		Object.fromEntries(
			Object.entries(fields).map(([name, value]) => [name, { value }]),
		),
	);
	target.dispatchEvent(event);
	return event.defaultPrevented;
};
type Harness = Readonly<{
	surface: Surface;
	keyboard: EventTarget;
	visibility: EventTarget;
	control: EventTarget;
	input: GizmoInput;
	changes: ScenePatch[];
	active: (Handle | undefined)[];
	errors: unknown[];
	pointer: (
		type: string,
		pixel: Pixel,
		shift?: boolean,
		id?: number,
	) => boolean;
	object: () => SceneObject;
	starts: () => number;
	select: (next: SceneObject | undefined) => void;
	hide: () => void;
	block: () => void;
	key: (key: string, shift?: boolean) => boolean;
}>;
// biome-ignore lint/complexity/noExcessiveLinesPerFunction: One fixture owns its listeners and mutable callback state.
const harness = (handle: Handle = { mode: "move", axis: 0 }): Harness => {
	const surface = new Surface();
	const keyboard = new EventTarget();
	const visibility = new EventTarget();
	const control = new EventTarget();
	let object = original;
	let selected = true;
	let hidden = false;
	let blocked = false;
	let starts = 0;
	const changes: ScenePatch[] = [];
	const active: (Handle | undefined)[] = [];
	const errors: unknown[] = [];
	const input: GizmoInput = listenGizmo({
		surface,
		keyboard,
		visibility,
		context: (): GizmoContext | undefined => {
			if (selected) {
				return { object, projection };
			}
			return;
		},
		handle: (target: EventTarget | null): Handle | undefined => {
			if (target === control) {
				return handle;
			}
			return;
		},
		hidden: () => hidden,
		blocked: () => blocked,
		start: () => {
			starts += 1;
		},
		change: (id: number, patch: ScenePatch): void => {
			if (id !== object.id) {
				throw new Error("Stale object update");
			}
			changes.push(patch);
			object = { ...object, ...patch };
			input.sync();
		},
		active: (value: Handle | undefined): void => {
			active.push(value);
		},
		error: (error: unknown): void => {
			errors.push(error);
		},
	});
	const pointer = (
		type: string,
		pixel: Pixel,
		shiftKey?: boolean,
		pointerId?: number,
	): boolean =>
		dispatch(surface, type, {
			target: control,
			pointerId: pointerId ?? 1,
			clientX: pixel[0] + 10,
			clientY: pixel[1] + 20,
			button: 0,
			shiftKey,
		});
	return {
		surface,
		keyboard,
		visibility,
		control,
		input,
		pointer,
		changes,
		active,
		errors,
		object: (): SceneObject => object,
		starts: (): number => starts,
		select: (next: SceneObject | undefined): void => {
			selected = next !== undefined;
			if (next !== undefined) {
				object = next;
			}
			input.sync();
		},
		hide: (): void => {
			hidden = true;
			visibility.dispatchEvent(new Event("visibilitychange"));
		},
		block: (): void => {
			blocked = true;
		},
		key: (key: string, shiftKey?: boolean): boolean =>
			dispatch(surface, "keydown", { target: control, key, shiftKey }),
	};
};

test("pointer capture isolates camera gestures and applies numeric Shift increments", () => {
	const h = harness();
	expect(h.pointer("pointerdown", [572, 400])).toBe(true);
	expect(h.surface.captured.has(1)).toBe(true);
	expect(h.starts()).toBe(1);
	h.pointer("pointermove", [586.4, 400]);
	expect(h.object().position[0]).toBeCloseTo(0.1, 12);
	h.pointer("pointermove", [600.8, 400], true, 9);
	expect(h.object().position[0]).toBeCloseTo(0.1, 12);
	h.pointer("pointermove", [600.8, 400], true);
	expect(h.object().position[0]).toBeCloseTo(0.3, 12);
	h.pointer("pointerup", [615.2, 400]);
	expect(h.object().position[0]).toBeCloseTo(0.4, 12);
	expect(h.input.dragging()).toBe(false);
	expect(h.surface.captured.size).toBe(0);
	const count = h.changes.length;
	h.pointer("pointermove", [629.6, 400]);
	expect(h.changes).toHaveLength(count);
	h.input.dispose();
});

test("rotation pointer events forward Shift and Escape restores the original pose", () => {
	const h = harness({ mode: "rotate", axis: 2 });
	const point = (angle: number): Pixel =>
		required(project(projection, arcPoint(original.position, 2, angle, 1)));
	h.pointer("pointerdown", point(0.2));
	h.pointer("pointermove", point(0.3));
	h.pointer("pointermove", point(0.4), true);
	const direction = rotateVector(
		h.object().rotation ?? IDENTITY_ROTATION,
		[1, 0, 0],
	);
	expect(direction[1]).toBeCloseTo(Math.sin(0.3), 12);
	expect(dispatch(h.keyboard, "keydown", { key: "Escape" })).toBe(true);
	expect(h.object().rotation).toEqual(IDENTITY_ROTATION);
	expect(h.surface.captured.size).toBe(0);
	h.input.dispose();
});

test("cancel, lost capture, blur, hidden and resize roll back once and release capture", () => {
	for (const reason of [
		"pointercancel",
		"lostpointercapture",
		"blur",
		"hidden",
		"resize",
	]) {
		const h = harness();
		h.pointer("pointerdown", [572, 400]);
		h.pointer("pointermove", [586.4, 400], true);
		expect(h.object().position[0]).toBeCloseTo(0.2, 12);
		if (reason === "hidden") {
			h.hide();
		} else if (reason === "blur" || reason === "resize") {
			h.keyboard.dispatchEvent(new Event(reason));
		} else {
			h.pointer(reason, [586.4, 400]);
		}
		expect(h.object().position).toEqual(original.position);
		expect(h.input.dragging()).toBe(false);
		expect(h.surface.captured.size).toBe(0);
		const count = h.changes.length;
		h.input.cancel();
		expect(h.changes).toHaveLength(count);
		h.input.dispose();
	}
});

test("keyboard handles nudge fine positions, honor Shift and ignore modified input", () => {
	const h = harness();
	expect(h.key("ArrowRight", true)).toBe(true);
	expect(h.object().position).toEqual([0.002, 0, 0]);
	expect(h.key("ArrowLeft")).toBe(true);
	expect(h.object().position[0]).toBe(0.001);
	expect(
		dispatch(h.surface, "keydown", {
			target: h.control,
			key: "ArrowRight",
			ctrlKey: true,
		}),
	).toBe(false);
	expect(h.object().position[0]).toBe(0.001);
	expect(h.key("Enter")).toBe(true);
	expect(h.key(" ", true)).toBe(true);
	expect(h.object().position[0]).toBe(0.004);
	expect(h.surface.captured.size).toBe(0);
	h.input.dispose();
	expect(h.key("ArrowUp")).toBe(false);
});

test("selection changes and disposal cannot restore into another object or leak listeners", () => {
	const h = harness();
	h.pointer("pointerdown", [572, 400]);
	h.pointer("pointermove", [586.4, 400]);
	h.select({ ...original, id: 2, position: [5, 0, 0] });
	expect(h.object().position).toEqual([5, 0, 0]);
	expect(h.surface.captured.size).toBe(0);
	h.input.cancel();
	expect(h.object().position).toEqual([5, 0, 0]);
	expect(h.errors).toHaveLength(0);
	h.input.dispose();
	h.input.dispose();
	expect(h.pointer("pointerdown", [572, 400])).toBe(false);
	expect(h.key("ArrowRight")).toBe(false);
	expect(h.input.dragging()).toBe(false);
});

test("blocked, invalid, secondary and failed-capture gestures fail closed", () => {
	const h = harness();
	h.surface.failCapture = true;
	h.pointer("pointerdown", [572, 400]);
	expect(h.errors).toHaveLength(1);
	expect(h.input.dragging()).toBe(false);
	h.surface.failCapture = false;
	expect(h.pointer("pointerdown", [Number.NaN, 400])).toBe(false);
	expect(
		dispatch(h.surface, "pointerdown", {
			target: h.control,
			pointerId: 1,
			clientX: 582,
			clientY: 420,
			button: 2,
		}),
	).toBe(false);
	h.block();
	expect(h.pointer("pointerdown", [572, 400])).toBe(false);
	expect(h.changes).toHaveLength(0);
	h.input.dispose();
});
