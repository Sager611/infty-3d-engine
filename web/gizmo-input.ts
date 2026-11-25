// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Vanilla event adapter, not Qwik.
import {
	advanceDrag,
	beginDrag,
	type Drag,
	type Handle,
	nudge,
	type Pixel,
	type Projection,
} from "./gizmo.ts";
import { modifiedInput } from "./keys.ts";
import { IDENTITY_ROTATION } from "./rotation.ts";
import type { SceneObject, ScenePatch } from "./scene.ts";

type PointerSurface = EventTarget &
	Pick<
		Element,
		"setPointerCapture" | "hasPointerCapture" | "releasePointerCapture"
	> & {
		getBoundingClientRect(): Pick<DOMRect, "left" | "top">;
	};
type GizmoContext = Readonly<{ object: SceneObject; projection: Projection }>;
type GizmoBindings = Readonly<{
	surface: PointerSurface;
	keyboard: EventTarget;
	visibility: EventTarget;
	context: () => GizmoContext | undefined;
	handle: (target: EventTarget | null) => Handle | undefined;
	hidden: () => boolean;
	blocked: () => boolean;
	start: (handle: Handle) => void;
	change: (id: number, patch: ScenePatch) => void;
	active: (handle: Handle | undefined) => void;
	error: (error: unknown) => void;
}>;
type GizmoInput = Readonly<{
	dragging: () => boolean;
	sync: () => void;
	cancel: (restore?: boolean) => void;
	dispose: () => void;
}>;
type Gesture = Readonly<{ pointerId: number; drag: Drag; dirty: boolean }>;
const numeric = (value: unknown): number | undefined => {
	if (typeof value === "number" && Number.isFinite(value)) {
		return value;
	}
	return;
};
const pointerNumber = (event: Event): number | undefined =>
	numeric("pointerId" in event && event.pointerId);
const boosted = (event: Event): boolean =>
	"shiftKey" in event && event.shiftKey === true;

// One captured transaction. Cancel restores its pose; replacement/disposal never writes stale state.
// biome-ignore lint/complexity/noExcessiveLinesPerFunction: Listener state and matching cleanup share one scope.
const listenGizmo = (bindings: GizmoBindings): GizmoInput => {
	const { surface } = bindings;
	let gesture: Gesture | undefined;
	let closed = false;
	const finish = (restore: boolean): void => {
		const current = gesture;
		gesture = undefined;
		if (current === undefined) {
			return;
		}
		bindings.active(undefined);
		if (surface.hasPointerCapture(current.pointerId)) {
			surface.releasePointerCapture(current.pointerId);
		}
		const context = bindings.context();
		if (
			restore &&
			current.dirty &&
			context?.object.id === current.drag.original.id
		) {
			bindings.change(context.object.id, {
				position: current.drag.original.position,
				rotation: current.drag.original.rotation ?? IDENTITY_ROTATION,
			});
		}
	};
	const sync = (): void => {
		if (gesture === undefined) {
			return;
		}
		if (bindings.hidden() || bindings.blocked()) {
			finish(true);
		} else if (bindings.context()?.object.id !== gesture.drag.original.id) {
			finish(false);
		}
	};
	const point = (event: Event): Pixel | undefined => {
		const x = numeric("clientX" in event && event.clientX);
		const y = numeric("clientY" in event && event.clientY);
		if (x === undefined || y === undefined) {
			return;
		}
		const rect = surface.getBoundingClientRect();
		return [x - rect.left, y - rect.top];
	};
	const down = (event: Event): void => {
		if (
			closed ||
			gesture !== undefined ||
			bindings.blocked() ||
			bindings.hidden() ||
			modifiedInput(event) ||
			!("button" in event) ||
			event.button !== 0 ||
			("isPrimary" in event && event.isPrimary === false)
		) {
			return;
		}
		const pointerId = pointerNumber(event);
		const pixel = point(event);
		const handle = bindings.handle(event.target);
		const context = bindings.context();
		if (
			pointerId === undefined ||
			!Number.isInteger(pointerId) ||
			pointerId < 0 ||
			pixel === undefined ||
			handle === undefined ||
			context === undefined
		) {
			return;
		}
		const drag = beginDrag(context.projection, context.object, handle, pixel);
		if (drag === undefined) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		gesture = { pointerId, drag, dirty: false };
		surface.setPointerCapture(pointerId);
		bindings.start(handle);
		bindings.active(handle);
	};
	const move = (event: Event): void => {
		sync();
		const current = gesture;
		if (current === undefined || pointerNumber(event) !== current.pointerId) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const pixel = point(event);
		const context = bindings.context();
		if (pixel === undefined || context === undefined) {
			return;
		}
		const next = advanceDrag(
			current.drag,
			context.object,
			pixel,
			boosted(event),
		);
		gesture = {
			...current,
			drag: next.drag,
			dirty: current.dirty || next.patch !== undefined,
		};
		if (next.patch !== undefined) {
			bindings.change(context.object.id, next.patch);
		}
	};
	const up = (event: Event): void => {
		if (gesture?.pointerId !== pointerNumber(event)) {
			return;
		}
		move(event);
		finish(false);
	};
	const cancelPointer = (event: Event): void => {
		if (gesture?.pointerId === pointerNumber(event)) {
			finish(true);
		}
	};
	const key = (event: Event): void => {
		if (
			closed ||
			event.defaultPrevented ||
			gesture !== undefined ||
			bindings.blocked() ||
			bindings.hidden() ||
			modifiedInput(event) ||
			!("key" in event) ||
			typeof event.key !== "string"
		) {
			return;
		}
		const handle = bindings.handle(event.target);
		const context = bindings.context();
		const amount =
			Number(
				event.key === "ArrowRight" ||
					event.key === "ArrowUp" ||
					event.key === "Enter" ||
					event.key === " ",
			) - Number(event.key === "ArrowLeft" || event.key === "ArrowDown");
		if (amount === 0 || handle === undefined || context === undefined) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const patch = nudge(
			context.object,
			handle,
			amount * (1 + Number(boosted(event))),
		);
		if (Object.keys(patch).length === 0) {
			return;
		}
		bindings.start(handle);
		bindings.change(context.object.id, patch);
	};
	const escapeKey = (event: Event): void => {
		if (gesture !== undefined && "key" in event && event.key === "Escape") {
			event.preventDefault();
			event.stopPropagation();
			finish(true);
		}
	};
	const guarded =
		(listener: EventListener): EventListener =>
		(event: Event): void => {
			try {
				listener(event);
			} catch (error) {
				finish(false);
				bindings.error(error);
			}
		};
	const listeners: readonly Readonly<
		[EventTarget, string, EventListener, boolean]
	>[] = [
		[surface, "pointerdown", guarded(down), false],
		[surface, "pointermove", guarded(move), false],
		[surface, "pointerup", guarded(up), false],
		[surface, "pointercancel", guarded(cancelPointer), false],
		[surface, "lostpointercapture", guarded(cancelPointer), false],
		[surface, "keydown", guarded(key), false],
		[bindings.keyboard, "keydown", guarded(escapeKey), true],
		[bindings.keyboard, "blur", guarded(() => finish(true)), false],
		[bindings.keyboard, "resize", guarded(() => finish(true)), false],
		[
			bindings.visibility,
			"visibilitychange",
			guarded(() => {
				if (bindings.hidden()) {
					finish(true);
				}
			}),
			false,
		],
	];
	for (const [target, name, listener, capture] of listeners) {
		target.addEventListener(name, listener, capture);
	}
	return {
		dragging: () => gesture !== undefined,
		sync,
		cancel: (restore?: boolean): void => finish(restore !== false),
		dispose: (): void => {
			if (closed) {
				return;
			}
			closed = true;
			finish(false);
			for (const [target, name, listener, capture] of listeners) {
				target.removeEventListener(name, listener, capture);
			}
		},
	};
};
export { listenGizmo, type GizmoContext, type GizmoInput, type GizmoBindings };
