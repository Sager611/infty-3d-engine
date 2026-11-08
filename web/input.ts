// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a vanilla DOM adapter.
// biome-ignore-all lint/style/noMagicNumbers: Input increments are shared interaction defaults.
// biome-ignore-all lint/nursery/noTernary: DOM target checks stay compact.
// biome-ignore-all lint/security/noSecrets: DOM property names are not secrets.
import { orbitCamera } from "./camera.ts";
import { normalizeCamera, type PureCamera } from "./state.ts";

const CLICK_THRESHOLD = 4;
const KEY_STEP = 0.08;
const WHEEL_SCALE = 0.001;

type InputBindings = Readonly<{
	canvas: HTMLCanvasElement;
	getCamera: () => PureCamera;
	setCamera: (camera: PureCamera) => void;
	zoom: (amount: number) => void;
	pick: (clientX: number, clientY: number, all: boolean) => void;
	probe: (clientX: number, clientY: number) => void;
	probeOpen: () => boolean;
	pickAll: () => boolean;
	reset: () => void;
	frame: () => void;
}>;

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

const editableTarget = (target: EventTarget | null): boolean => {
	if (target === null || typeof target !== "object") {
		return false;
	}
	const tagName =
		"tagName" in target && typeof target.tagName === "string"
			? target.tagName.toUpperCase()
			: "";
	if (
		tagName === "INPUT" ||
		tagName === "TEXTAREA" ||
		tagName === "SELECT" ||
		tagName === "BUTTON" ||
		tagName === "OPTION"
	) {
		return true;
	}
	return "isContentEditable" in target && target.isContentEditable === true;
};

const keyboardCamera = (
	camera: PureCamera,
	yawDelta: number,
	pitchDelta: number,
): PureCamera =>
	normalizeCamera({
		...camera,
		yaw: camera.yaw + yawDelta,
		pitch: camera.pitch + pitchDelta,
	});

type KeyAction =
	| "left"
	| "right"
	| "up"
	| "down"
	| "zoomIn"
	| "zoomOut"
	| "reset";

const keyAction = (key: string): KeyAction | undefined => {
	if (key === "ArrowLeft") {
		return "left";
	}
	if (key === "ArrowRight") {
		return "right";
	}
	if (key === "ArrowUp") {
		return "up";
	}
	if (key === "ArrowDown") {
		return "down";
	}
	if (key === "+" || key === "=") {
		return "zoomIn";
	}
	if (key === "-" || key === "_") {
		return "zoomOut";
	}
	if (key.toLowerCase() === "r") {
		return "reset";
	}
	return;
};

const handleKeydown = (bindings: InputBindings, event: KeyboardEvent): void => {
	if (editableTarget(event.target)) {
		return;
	}
	const action = keyAction(event.key);
	if (action === undefined) {
		return;
	}
	if (action === "left") {
		bindings.setCamera(keyboardCamera(bindings.getCamera(), -KEY_STEP, 0));
	} else if (action === "right") {
		bindings.setCamera(keyboardCamera(bindings.getCamera(), KEY_STEP, 0));
	} else if (action === "up") {
		bindings.setCamera(keyboardCamera(bindings.getCamera(), 0, -KEY_STEP));
	} else if (action === "down") {
		bindings.setCamera(keyboardCamera(bindings.getCamera(), 0, KEY_STEP));
	} else if (action === "zoomIn") {
		bindings.zoom(-KEY_STEP);
	} else if (action === "zoomOut") {
		bindings.zoom(KEY_STEP);
	} else {
		bindings.reset();
	}
	event.preventDefault();
};

// biome-ignore lint/complexity/noExcessiveLinesPerFunction: The adapter keeps all listener state in one cleanup scope.
const bindInput = (bindings: InputBindings): (() => void) => {
	const { canvas } = bindings;
	let gesture: Gesture | undefined;
	const pointers = new Set<number>();
	let multiTouch = false;

	const releaseCapture = (pointerId: number): void => {
		if (canvas.hasPointerCapture(pointerId)) {
			canvas.releasePointerCapture(pointerId);
		}
	};

	const pointerdown = (event: PointerEvent): void => {
		if (pointers.has(event.pointerId)) {
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
		const validStart =
			Number.isFinite(event.clientX) && Number.isFinite(event.clientY);
		gesture = {
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			startCamera: bindings.getCamera(),
			moved: false,
			cancelled: !validStart,
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
			bindings.setCamera(orbitCamera(gesture.startCamera, dx, dy));
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
			releaseCapture(event.pointerId);
			gesture = undefined;
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
		event.preventDefault();
		bindings.zoom(event.deltaY * WHEEL_SCALE);
	};

	const keydown = (event: KeyboardEvent): void => {
		handleKeydown(bindings, event);
	};

	canvas.addEventListener("pointerdown", pointerdown);
	canvas.addEventListener("pointermove", pointermove);
	canvas.addEventListener("pointerup", pointerup);
	canvas.addEventListener("pointercancel", pointercancel);
	canvas.addEventListener("wheel", wheel, { passive: false });
	canvas.addEventListener("keydown", keydown);

	return (): void => {
		canvas.removeEventListener("pointerdown", pointerdown);
		canvas.removeEventListener("pointermove", pointermove);
		canvas.removeEventListener("pointerup", pointerup);
		canvas.removeEventListener("pointercancel", pointercancel);
		canvas.removeEventListener("wheel", wheel);
		canvas.removeEventListener("keydown", keydown);
		if (gesture !== undefined) {
			releaseCapture(gesture.pointerId);
		}
		gesture = undefined;
		pointers.clear();
		multiTouch = false;
	};
};

export { bindInput, isClick, editableTarget, type InputBindings };
