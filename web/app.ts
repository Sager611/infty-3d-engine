// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Vanilla DOM coordination.
import init, { PreparedScene, Renderer } from "../pkg/infty.js";
import {
	cameraBasis,
	frameCamera,
	type Movement,
	moveCamera,
	STOPPED,
	stepSpeed,
	travelSpeed,
} from "./camera.ts";
import {
	type ControlsEvent,
	type ControlsNodes,
	type ControlsState,
	controlsNodes,
	listenControls,
	propertyPatch,
	renderControls,
} from "./controls.ts";
import {
	addInstance,
	cleanSelection,
	createEditor,
	deleteInstances,
	type Editor,
	undoDeletion,
} from "./editing.ts";
import { createFrames, type Frames } from "./frames.ts";
import {
	bindInput,
	type InputBindings,
	type InputController,
} from "./input.ts";
import { type Meter, startMetrics } from "./metrics.ts";
import {
	acquireResource,
	emptySceneCache,
	prepareScene,
	releaseScene,
	type SceneCache,
} from "./prepared.ts";
import {
	activateSelection,
	canvasPixel,
	decodeQueries,
	EMPTY_SELECTION,
	inspectResult,
	type QueryResult,
	type Selection,
	selectionFromHits,
} from "./query.ts";
import { updateObject } from "./scene.ts";
import { listenSceneActions, type SceneAction } from "./scene-actions.ts";
import {
	cameraPacket,
	DEFAULT_SNAPSHOT,
	normalizeCamera,
	parseSnapshot,
	type Snapshot,
	serializeSnapshot,
} from "./state.ts";
import {
	EMPTY_STATUS,
	renderedStatus,
	type Status,
	statusMessage,
} from "./status.ts";

const INITIAL_SPEED = 3;
const SPEED_DIGITS = 3;
const MILLISECONDS_PER_SECOND = 1000;
const MIN_SIZE = 1;
const MAX_WIDTH = 1440;
const MAX_HEIGHT = 1000;
const MAX_PIXELS = 1_000_000;
const HASH_DELAY = 150;
const canvas: HTMLElement | null = document.getElementById("viewport");
const modeNode: HTMLElement | null = document.getElementById("mode");
const resetNode: HTMLElement | null = document.getElementById("reset");
const focusNode: HTMLElement | null = document.getElementById("focus");
const metricsNode: HTMLElement | null = document.getElementById("metrics");
const inspectorNode: HTMLElement | null = document.getElementById("inspector");
const statusNode: HTMLElement | null = document.getElementById("status");
if (
	!(
		canvas instanceof HTMLCanvasElement &&
		modeNode instanceof HTMLSelectElement &&
		resetNode instanceof HTMLButtonElement &&
		focusNode instanceof HTMLButtonElement &&
		metricsNode instanceof HTMLElement &&
		inspectorNode instanceof HTMLElement &&
		statusNode instanceof HTMLElement
	)
) {
	throw new Error("Renderer controls are missing.");
}
const controls: ControlsNodes = controlsNodes(document);
const metrics: HTMLElement = metricsNode;
const inspector: HTMLElement = inspectorNode;
const status: HTMLElement = statusNode;
const toolsNode: HTMLElement | null = document.getElementById("scene-tools");
const addMenu: HTMLElement | null = document.getElementById("add-menu");
const deleteButton: HTMLElement | null =
	document.getElementById("delete-objects");
const undoButton: HTMLElement | null = document.getElementById("undo-delete");
const deleteCount: HTMLElement | null = document.getElementById("delete-count");
const sceneDetails: HTMLElement | null =
	document.getElementById("scene-controls");
if (
	!(
		toolsNode instanceof HTMLElement &&
		addMenu instanceof HTMLElement &&
		deleteButton instanceof HTMLButtonElement &&
		undoButton instanceof HTMLButtonElement &&
		deleteCount instanceof HTMLElement &&
		sceneDetails instanceof HTMLDetailsElement
	)
) {
	throw new Error("Scene tools missing");
}

let snapshot: Snapshot;
let hashError: unknown;
try {
	snapshot = parseSnapshot(location.hash);
} catch (error) {
	snapshot = DEFAULT_SNAPSHOT;
	hashError = error;
}
let selection: Selection = EMPTY_SELECTION;
let editor: Editor = createEditor(snapshot.objects);
let pickAll = false;
let renderer: Renderer | undefined;
let prepared: SceneCache<PreparedScene> = emptySceneCache<PreparedScene>();
let wasmMemory: WebAssembly.Memory | undefined;
let meter: Meter | undefined;
let movement: Movement = STOPPED;
let movementTime: number | undefined;
let baseSpeed: number = INITIAL_SPEED;
let saveTimer: number | undefined;
let disposed = false;
let outputStatus: Status = EMPTY_STATUS;

const errorMessage = (error: unknown): string => {
	if (error instanceof Error) {
		return error.message;
	}
	return String(error);
};

const setStatus = (
	message: string,
	source: Status["source"] = "action",
): void => {
	if (outputStatus.text === message && outputStatus.source === source) {
		return;
	}
	outputStatus = statusMessage(message, source);
	status.textContent = outputStatus.text;
};

const stateForControls = (): ControlsState => ({
	objects: snapshot.objects,
	selection,
	pickAll,
});

const updateUi = (): void => {
	modeNode.value = String(snapshot.mode);
	renderControls(controls, stateForControls());
	deleteButton.disabled = selection.ids.length === 0;
	undoButton.disabled = editor.undo === null;
	deleteCount.textContent = String(selection.ids.length);
	const label = `Delete ${selection.ids.length} selected objects`;
	deleteButton.ariaLabel = label;
	deleteButton.title = label;
	deleteButton.dataset.tip = label;
};

const scheduleHash = (): void => {
	if (saveTimer !== undefined) {
		window.clearTimeout(saveTimer);
	}
	saveTimer = window.setTimeout(() => {
		saveTimer = undefined;
		const hash = serializeSnapshot(snapshot);
		if (location.hash !== hash) {
			history.replaceState(null, "", hash);
		}
	}, HASH_DELAY);
};

const invalidate = (): void => {
	if (disposed) {
		return;
	}
	scheduleHash();
	frames.invalidate();
};

const hasMovement = (value: Movement): boolean =>
	value.forward !== 0 || value.side !== 0;
const setMovement = (value: Movement): void => {
	if (
		disposed ||
		(movement.forward === value.forward &&
			movement.side === value.side &&
			movement.onlyForward === value.onlyForward)
	) {
		return;
	}
	if (!hasMovement(value)) {
		movementTime = undefined;
	} else if (!hasMovement(movement)) {
		movementTime = performance.now();
	}
	movement = value;
	if (hasMovement(value)) {
		setStatus("");
	}
	invalidate();
};

const setSnapshot = (next: Snapshot): void => {
	if (snapshot.objects !== next.objects) {
		editor = { ...editor, objects: next.objects, undo: null };
		selection = cleanSelection(next.objects, selection);
		inspector.textContent = "Move pointer over the canvas to inspect.";
	}
	snapshot = next;
	updateUi();
	setStatus("");
	invalidate();
};

const setCamera = (camera: Snapshot["camera"]): void => {
	snapshot = { ...snapshot, camera: normalizeCamera(camera) };
	setStatus("");
	invalidate();
};

const resetDefaults = (): void => {
	removeInput?.stop();
	baseSpeed = INITIAL_SPEED;
	metrics.removeAttribute("title");
	editor = createEditor(DEFAULT_SNAPSHOT.objects);
	selection = EMPTY_SELECTION;
	pickAll = false;
	setSnapshot(DEFAULT_SNAPSHOT);
	inspector.textContent = "Move pointer over the canvas to inspect.";
};

const readMode = (): 0 | 1 | 2 => {
	const value = Number(modeNode.value);
	if (value !== 0 && value !== 1 && value !== 2) {
		throw new Error("invalid display mode");
	}
	return value;
};

const resizeCanvas = (): void => {
	if (renderer === undefined) {
		return;
	}
	const cssWidth = Math.max(MIN_SIZE, canvas.clientWidth);
	const cssHeight = Math.max(MIN_SIZE, canvas.clientHeight);
	const dpr = Math.max(MIN_SIZE, window.devicePixelRatio || MIN_SIZE);
	const pixelScale = Math.min(
		dpr,
		Math.sqrt(MAX_PIXELS / (cssWidth * cssHeight)),
		MAX_WIDTH / cssWidth,
		MAX_HEIGHT / cssHeight,
	);
	const width = Math.max(
		MIN_SIZE,
		Math.min(MAX_WIDTH, Math.round(cssWidth * pixelScale)),
	);
	const height = Math.max(
		MIN_SIZE,
		Math.min(MAX_HEIGHT, Math.round(cssHeight * pixelScale)),
	);
	if (canvas.width === width && canvas.height === height) {
		return;
	}
	canvas.width = width;
	canvas.height = height;
	renderer.resize(width, height);
};

const cameraValues = (): Float64Array =>
	new Float64Array(cameraPacket(snapshot.camera));

const createScene = (packet: Float64Array): PreparedScene =>
	new PreparedScene(packet);

const preparedScene = (): PreparedScene => {
	if (disposed || wasmMemory === undefined) {
		throw new Error("scene unavailable");
	}
	prepared = prepareScene(prepared, snapshot.objects, createScene);
	if (prepared.value === null) {
		throw new Error("scene unavailable");
	}
	return prepared.value;
};

const screenPoint = (clientX: number, clientY: number): Float64Array => {
	const [x, y] = canvasPixel(
		[canvas.width, canvas.height],
		canvas.getBoundingClientRect(),
		[clientX, clientY],
	);
	return new Float64Array([canvas.width, canvas.height, x, y]);
};

const queryAt = (
	clientX: number,
	clientY: number,
	mode: 0 | 1 | 2,
): readonly QueryResult[] => {
	if (renderer === undefined) {
		throw new Error("renderer unavailable");
	}
	return decodeQueries(
		preparedScene().query_camera(
			cameraValues(),
			screenPoint(clientX, clientY),
			mode,
		),
	);
};

const probe = (clientX: number, clientY: number): void => {
	if (!inspector.closest("details")?.open) {
		return;
	}
	try {
		const results = queryAt(clientX, clientY, 2);
		inspector.textContent = "No query results.";
		if (results.length > 0) {
			inspector.textContent = results.map(inspectResult).join("\n");
		}
	} catch (error) {
		inspector.textContent = `Inspect failed: ${errorMessage(error)}`;
	}
};

const pick = (clientX: number, clientY: number, all: boolean): void => {
	try {
		let queryMode: 0 | 1 = 0;
		if (all) {
			queryMode = 1;
		}
		const results = queryAt(clientX, clientY, queryMode);
		selection = selectionFromHits(results);
		updateUi();
		setStatus("");
		invalidate();
	} catch (error) {
		updateUi();
		setStatus(`Pick failed: ${errorMessage(error)}`);
	}
};

const centerHitDistance = (): number | null => {
	const basis = cameraBasis(snapshot.camera);
	const [result] = decodeQueries(
		preparedScene().query_ray(
			new Float64Array([...basis.origin, ...basis.forward]),
			0,
		),
	);
	if (result?.uncertain === true) {
		throw new Error("Center-ray hit is unresolved");
	}
	if (result !== undefined && !result.hit) {
		throw new Error("Center-ray query returned a non-hit");
	}
	return result?.t ?? null;
};

const speed = (amount: number): void => {
	baseSpeed = travelSpeed(baseSpeed, amount);
	setStatus("");
	metrics.title = `Base travel speed ${baseSpeed.toPrecision(SPEED_DIGITS)} units/s`;
};

const frame = (): void => {
	removeInput?.stop();
	try {
		const aspect =
			Math.max(MIN_SIZE, canvas.width) / Math.max(MIN_SIZE, canvas.height);
		setCamera(
			frameCamera(snapshot.camera, snapshot.objects, selection.ids, aspect),
		);
	} catch (error) {
		updateUi();
		setStatus(`Frame failed: ${errorMessage(error)}`);
	}
};

const handleControls = (event: ControlsEvent): void => {
	try {
		if (event.kind === "select") {
			if (event.id === null) {
				selection = EMPTY_SELECTION;
			} else if (!snapshot.objects.some((item) => item.id === event.id)) {
				throw new Error("unknown object");
			} else if (selection.ids.includes(event.id)) {
				selection = activateSelection(selection, event.id);
			} else {
				selection = Object.freeze({
					ids: Object.freeze([event.id]),
					active: event.id,
				});
			}
			updateUi();
			invalidate();
			return;
		}
		if (event.kind === "pickAll") {
			pickAll = event.value;
			updateUi();
			invalidate();
			return;
		}
		const object = snapshot.objects.find((item) => item.id === event.id);
		if (object === undefined) {
			throw new Error("unknown object");
		}
		const objects = updateObject(
			snapshot.objects,
			event.id,
			propertyPatch(object, event.key, event.value),
		);
		setSnapshot({ ...snapshot, objects });
	} catch (error) {
		updateUi();
		setStatus(`Control update failed: ${errorMessage(error)}`);
	}
};

const renderFrame = (now: number): void => {
	if (renderer === undefined || disposed || document.hidden) {
		return;
	}
	let phase: Status["source"] = "action";
	try {
		const started = performance.now();
		if (hasMovement(movement)) {
			let seconds = 0;
			if (movementTime !== undefined) {
				seconds = Math.max(0, (now - movementTime) / MILLISECONDS_PER_SECOND);
			}
			movementTime = now;
			snapshot = {
				...snapshot,
				camera: moveCamera(
					snapshot.camera,
					movement,
					stepSpeed(movement, baseSpeed, seconds, centerHitDistance),
					seconds,
				),
			};
			scheduleHash();
		}
		phase = "render";
		resizeCanvas();
		renderer.render_scene(
			cameraValues(),
			preparedScene(),
			snapshot.mode,
			new Uint32Array(selection.ids),
		);
		meter?.record(performance.now() - started);
		if (renderedStatus(outputStatus) !== outputStatus) {
			setStatus("");
		}
	} catch (error) {
		removeInput?.stop();
		setStatus(`View update failed: ${errorMessage(error)}`, phase);
	}
};

const frames: Frames = createFrames({
	request: (tick: FrameRequestCallback) => window.requestAnimationFrame(tick),
	cancel: (id: number) => window.cancelAnimationFrame(id),
	visible: () => !(disposed || document.hidden),
	moving: () => hasMovement(movement),
	draw: renderFrame,
});

const hashChanged = (): void => {
	removeInput?.stop();
	try {
		snapshot = parseSnapshot(location.hash);
		editor = createEditor(snapshot.objects);
		selection = EMPTY_SELECTION;
		pickAll = false;
		inspector.textContent = "Move pointer over the canvas to inspect.";
		updateUi();
		invalidate();
		setStatus("");
	} catch (error) {
		updateUi();
		setStatus(`Invalid snapshot: ${errorMessage(error)}`);
	}
};

const resized = (): void => {
	try {
		resizeCanvas();
		invalidate();
	} catch (error) {
		setStatus(`Resize failed: ${errorMessage(error)}`);
	}
};

const visibilityChanged = (): void => {
	if (document.hidden) {
		removeInput?.stop();
		movementTime = undefined;
	}
	frames.visibility();
};

const cleanup = (): void => {
	if (disposed) {
		return;
	}
	disposed = true;
	if (saveTimer !== undefined) {
		window.clearTimeout(saveTimer);
	}
	frames.dispose();
	meter?.dispose();
	renderer?.free();
	prepared = releaseScene(prepared);
	removeControls?.();
	removeSceneActions?.();
	addMenu.removeEventListener("toggle", focusShape);
	removeInput?.dispose();
	window.removeEventListener("resize", resized);
	window.removeEventListener("hashchange", hashChanged);
	document.removeEventListener("visibilitychange", visibilityChanged);
};

const focusShape = (): void => {
	if (addMenu.matches(":popover-open")) {
		addMenu.querySelector<HTMLButtonElement>("button")?.focus();
	}
};
const handleSceneAction = (action: SceneAction): void => {
	try {
		const current = { ...editor, selection };
		if (action === "delete") {
			editor = deleteInstances(current);
		} else if (action === "undo") {
			editor = undoDeletion(current);
		} else {
			editor = addInstance(current, action, {
				...snapshot.camera,
				aspect: canvas.width / canvas.height,
			});
			addMenu.hidePopover();
			sceneDetails.open = true;
		}
		selection = editor.selection;
		snapshot = { ...snapshot, objects: editor.objects };
		inspector.textContent = "Move pointer over the canvas to inspect.";
		updateUi();
		invalidate();
		if (action !== "delete" && action !== "undo") {
			controls.properties.querySelector<HTMLInputElement>("input")?.focus();
		} else {
			canvas.focus();
		}
	} catch (error) {
		setStatus(errorMessage(error));
	}
};
let removeSceneActions: (() => void) | undefined;
let removeControls: (() => void) | undefined;
let removeInput: InputController | undefined;

const acceptRenderer = (value: Renderer): boolean => {
	if (disposed) {
		return false;
	}
	renderer = value;
	return true;
};

const start = async (): Promise<void> => {
	try {
		const wasm = await init();
		if (disposed) {
			return;
		}
		wasmMemory = wasm.memory;
		await acquireResource(() => Renderer.create(canvas), acceptRenderer);
		if (disposed) {
			return;
		}
		resizeCanvas();
		meter = startMetrics(metrics, wasmMemory);
		invalidate();
	} catch (error) {
		setStatus(`WebGPU initialization failed: ${errorMessage(error)}`);
	}
};

const inputBindings: InputBindings = {
	canvas,
	getCamera: () => snapshot.camera,
	setCamera,
	speed,
	movement: setMovement,
	keyboard: window,
	visibility: document,
	hidden: () => document.hidden,
	blocked: () =>
		disposed || renderer === undefined || addMenu.matches(":popover-open"),
	onError: (error: unknown) => {
		removeInput?.stop();
		setStatus(`Camera control failed: ${errorMessage(error)}`);
	},
	pick,
	probe,
	probeOpen: () => inspector.closest("details")?.open === true,
	pickAll: () => pickAll,
	reset: resetDefaults,
	frame,
};

addMenu.addEventListener("toggle", focusShape);
removeSceneActions = listenSceneActions(
	toolsNode,
	window,
	() => addMenu.matches(":popover-open"),
	handleSceneAction,
);
removeControls = listenControls(controls, stateForControls, handleControls);
removeInput = bindInput(inputBindings);
modeNode.addEventListener("change", () => {
	try {
		setSnapshot({ ...snapshot, mode: readMode() });
	} catch (error) {
		updateUi();
		setStatus(`Mode update failed: ${errorMessage(error)}`);
	}
});
resetNode.addEventListener("click", resetDefaults);
focusNode.addEventListener("click", frame);
window.addEventListener("resize", resized);
window.addEventListener("hashchange", hashChanged);
document.addEventListener("visibilitychange", visibilityChanged);
window.addEventListener("beforeunload", cleanup);
updateUi();
if (hashError !== undefined) {
	setStatus(`Invalid snapshot: ${errorMessage(hashError)}`);
}
await start();
