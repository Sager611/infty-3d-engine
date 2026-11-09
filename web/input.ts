// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a vanilla DOM adapter.
import { lookCamera } from "./camera.ts";
import {
	type KeyBindings,
	type KeyController,
	listenKeys,
	modifiedInput,
} from "./keys.ts";
import type { PureCamera } from "./state.ts";

const CLICK_THRESHOLD = 4;
const WHEEL_SCALE = 0.001;
type InputBindings = KeyBindings &
	Readonly<{
		canvas: HTMLCanvasElement;
		pick: (clientX: number, clientY: number, all: boolean) => void;
		probe: (clientX: number, clientY: number) => void;
		probeOpen: () => boolean;
		pickAll: () => boolean;
	}>;
type InputController = KeyController;
interface Gesture {
	pointerId: number;
	startX: number;
	startY: number;
	startCamera: PureCamera;
	moved: boolean;
	cancelled: boolean;
}
const isClick = (dx: number, dy: number): boolean =>
	Number.isFinite(dx) &&
	Number.isFinite(dy) &&
	Math.hypot(dx, dy) < CLICK_THRESHOLD;

const LINE_HEIGHT = 16;
function wheelAmount(delta: number, mode: number, height: number): number {
	if (
		!(Number.isFinite(delta) && Number.isFinite(height)) ||
		height <= 0 ||
		(mode !== 0 && mode !== 1 && mode !== 2)
	) {
		throw new Error("Invalid wheel input");
	}
	let pixels = delta;
	if (mode === 1) {
		pixels *= LINE_HEIGHT;
	}
	if (mode === 2) {
		pixels *= height;
	}
	const amount = pixels * WHEEL_SCALE;
	if (!Number.isFinite(amount)) {
		throw new Error("Wheel input overflow");
	}
	return amount;
}

// biome-ignore lint/complexity/noExcessiveLinesPerFunction: The adapter keeps all listener state in one cleanup scope.
const bindInput = (bindings: InputBindings): InputController => {
	const { canvas } = bindings;
	const keys = listenKeys(bindings);
	let gesture: Gesture | undefined;
	const pointers = new Set<number>();
	let multiTouch = false;
	let disposed = false;

	const releaseCapture = (pointerId: number): void => {
		if (canvas.hasPointerCapture(pointerId)) {
			canvas.releasePointerCapture(pointerId);
		}
	};
	const clearGesture = (): void => {
		const current = gesture;
		gesture = undefined;
		pointers.clear();
		multiTouch = false;
		if (current !== undefined) {
			releaseCapture(current.pointerId);
		}
	};
	const stop = (): void => {
		keys.stop();
		clearGesture();
	};
	const pointerdown = (event: PointerEvent): void => {
		if (bindings.blocked() || pointers.has(event.pointerId)) {
			return;
		}
		pointers.add(event.pointerId);
		if (pointers.size > 1) {
			multiTouch = true;
			if (gesture !== undefined) {
				gesture.cancelled = true;
			}
			return;
		}
		gesture = {
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			startCamera: bindings.getCamera(),
			moved: false,
			cancelled: !(
				Number.isFinite(event.clientX) && Number.isFinite(event.clientY)
			),
		};
		canvas.setPointerCapture(event.pointerId);
		canvas.focus();
	};
	const pointermove = (event: PointerEvent): void => {
		if (bindings.probeOpen()) {
			bindings.probe(event.clientX, event.clientY);
		}
		if (gesture?.pointerId !== event.pointerId || gesture.cancelled) {
			return;
		}
		const dx = event.clientX - gesture.startX;
		const dy = event.clientY - gesture.startY;
		if (!(Number.isFinite(dx) && Number.isFinite(dy))) {
			gesture.cancelled = true;
			return;
		}
		if (!isClick(dx, dy)) {
			gesture.moved = true;
			try {
				bindings.setCamera(
					lookCamera(gesture.startCamera, dx, dy, bindings.getCamera()),
				);
			} catch (error) {
				stop();
				bindings.onError(error);
			}
		}
	};
	const finishPointer = (event: PointerEvent, cancelled: boolean): void => {
		if (!pointers.delete(event.pointerId)) {
			return;
		}
		const current = gesture;
		if (current?.pointerId === event.pointerId) {
			if (
				!(cancelled || current.cancelled || multiTouch || current.moved) &&
				isClick(event.clientX - current.startX, event.clientY - current.startY)
			) {
				bindings.pick(
					event.clientX,
					event.clientY,
					event.altKey || bindings.pickAll(),
				);
			}
			gesture = undefined;
			releaseCapture(event.pointerId);
		}
		if (pointers.size === 0) {
			multiTouch = false;
		}
	};
	const pointerup = (event: PointerEvent): void => {
		finishPointer(event, false);
	};
	const pointercancel = (event: PointerEvent): void => {
		finishPointer(event, true);
	};
	const wheel = (event: WheelEvent): void => {
		if (modifiedInput(event) || bindings.blocked()) {
			return;
		}
		event.preventDefault();
		try {
			bindings.speed(
				wheelAmount(event.deltaY, event.deltaMode, canvas.clientHeight),
			);
		} catch (error) {
			stop();
			bindings.onError(error);
		}
	};

	canvas.addEventListener("pointerdown", pointerdown);
	canvas.addEventListener("pointermove", pointermove);
	canvas.addEventListener("pointerup", pointerup);
	canvas.addEventListener("pointercancel", pointercancel);
	canvas.addEventListener("lostpointercapture", pointercancel);
	canvas.addEventListener("wheel", wheel, { passive: false });
	bindings.keyboard.addEventListener("blur", stop);
	const dispose = (): void => {
		if (disposed) {
			return;
		}
		disposed = true;
		keys.dispose();
		clearGesture();
		canvas.removeEventListener("pointerdown", pointerdown);
		canvas.removeEventListener("pointermove", pointermove);
		canvas.removeEventListener("pointerup", pointerup);
		canvas.removeEventListener("pointercancel", pointercancel);
		canvas.removeEventListener("lostpointercapture", pointercancel);
		canvas.removeEventListener("wheel", wheel);
		bindings.keyboard.removeEventListener("blur", stop);
	};
	return Object.freeze({ stop, dispose });
};
export {
	bindInput,
	isClick,
	wheelAmount,
	type InputBindings,
	type InputController,
};
