// biome-ignore-all lint/nursery/useExplicitType: Strict TypeScript inference avoids contradictory redundant annotations.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is vanilla DOM, not Qwik.
import init, { Renderer } from "../pkg/infty.js";
import {
	ANGLE_LIMIT,
	DEFAULT_VIEW,
	MAX_DISTANCE,
	MAX_RADIUS,
	MIN_DISTANCE_GAP,
	MIN_RADIUS,
	parseView,
	serializeView,
	type View,
	type ViewMode,
	YAW_LIMIT,
} from "./view.ts";

interface Sample {
	hit: boolean;
	t: number;
	gap: number;
	u: number;
	v: number;
}
interface Pointer {
	x: number;
	y: number;
}
const browserPerformance: Performance & {
	readonly memory?: Readonly<Record<"usedJSHeapSize", number>>;
} = performance;
const MIN_CANVAS_SIZE = 1;
const MAX_CANVAS_WIDTH = 1440;
const MAX_CANVAS_HEIGHT = 1000;
const HIT_THRESHOLD = 0.5;
const SAMPLE_STRIDE = 16;
const UV_U_OFFSET = 3;
const UV_V_OFFSET = 4;
const MIN_PROBE_VALUES = SAMPLE_STRIDE * 2;
const MS_PER_SECOND = 1000;
const FOCUS_DISTANCE_FACTOR = 3;
const HASH_SAVE_DELAY_MS = 150;
const POINTER_SCALE = 0.008;
const KEY_STEP = 0.08;
const WHEEL_SCALE = 0.001;
const BYTES_PER_MIB = Number("1048576");
const METRICS_INTERVAL_MS = Number("1000");

const canvasNode = document.getElementById("viewport");
const radiusNode = document.getElementById("radius");
const radiusValueNode = document.getElementById("radius-value");
const modeNode = document.getElementById("mode");
const resetNode = document.getElementById("reset");
const focusNode = document.getElementById("focus");
const metricsNode = document.getElementById("metrics");
const inspectorNode = document.getElementById("inspector");
const statusNode = document.getElementById("status");

if (
	!(
		canvasNode instanceof HTMLCanvasElement &&
		radiusNode instanceof HTMLInputElement &&
		radiusValueNode instanceof HTMLOutputElement &&
		modeNode instanceof HTMLSelectElement &&
		resetNode instanceof HTMLButtonElement &&
		focusNode instanceof HTMLButtonElement &&
		metricsNode instanceof HTMLOutputElement &&
		inspectorNode instanceof HTMLOutputElement &&
		statusNode instanceof HTMLOutputElement
	)
) {
	throw new Error("Renderer controls are missing.");
}

const canvas = canvasNode;
const radiusInput = radiusNode;
const radiusOutput = radiusValueNode;
const modeSelect = modeNode;
const resetButton = resetNode;
const focusButton = focusNode;
const metrics = metricsNode;
const inspector = inspectorNode;
const status = statusNode;

const initialView = parseView(location.hash);

let renderer: Renderer | undefined;
let wasmMemory: WebAssembly.Memory | undefined;
let wasmStatus: "loading" | "available" | "unavailable" = "loading";
let framePending = false;
let pointerId: number | undefined;
let previousX = 0;
let previousY = 0;
let lastPointer: Pointer | undefined;
let yaw = initialView.yaw;
let pitch = initialView.pitch;
let distance = initialView.distance;
let radius = initialView.radius;
let mode: ViewMode = initialView.mode;
let renderedFrames = 0;
let metricsFrames = 0;
let metricsAt = performance.now();
let cpuElapsedMs: number | undefined;
let metricsTimer: number | undefined;
let saveTimer: number | undefined;

const clamp = (value: number, low: number, high: number): number => {
	if (Number.isFinite(value)) {
		return Math.min(high, Math.max(low, value));
	}
	return (low + high) / 2;
};

const format = (value: number, digits = 3): string => {
	if (Number.isFinite(value)) {
		return value.toFixed(digits);
	}
	return "—";
};

const readMode = (): ViewMode => {
	if (modeSelect.value === "2") {
		return 2;
	}
	if (modeSelect.value === "1") {
		return 1;
	}
	return 0;
};

const errorMessage = (error: unknown): string => {
	if (error instanceof Error) {
		return error.message;
	}
	return String(error);
};

const toMiB = (bytes: number): string => (bytes / BYTES_PER_MIB).toFixed(1);

const setStatus = (message: string): void => {
	status.textContent = message;
};

const readView = (): View => ({ yaw, pitch, distance, radius, mode });

const applyView = (view: View): void => {
	yaw = view.yaw;
	pitch = view.pitch;
	distance = view.distance;
	radius = view.radius;
	mode = view.mode;
};

const updateControls = (): void => {
	radiusInput.value = radius.toString();
	radiusOutput.value = radius.toFixed(2);
	modeSelect.value = mode.toString();
	focusButton.hidden = mode !== 2;
};

const saveView = (): void => {
	saveTimer = undefined;
	const hash = serializeView(readView());
	if (location.hash !== hash) {
		history.replaceState(null, "", hash);
	}
};

const scheduleSave = (): void => {
	if (saveTimer !== undefined) {
		window.clearTimeout(saveTimer);
	}
	saveTimer = window.setTimeout(saveView, HASH_SAVE_DELAY_MS);
};

const resetView = (): void => {
	applyView(DEFAULT_VIEW);
	updateControls();
	invalidate();
};

const resizeCanvas = (): void => {
	if (!renderer) {
		return;
	}
	const cssWidth = Math.max(MIN_CANVAS_SIZE, canvas.clientWidth);
	const cssHeight = Math.max(MIN_CANVAS_SIZE, canvas.clientHeight);
	const dpr = Math.max(
		MIN_CANVAS_SIZE,
		window.devicePixelRatio || MIN_CANVAS_SIZE,
	);
	const scale = Math.min(
		dpr,
		MAX_CANVAS_WIDTH / cssWidth,
		MAX_CANVAS_HEIGHT / cssHeight,
	);
	const width = Math.max(MIN_CANVAS_SIZE, Math.round(cssWidth * scale));
	const height = Math.max(MIN_CANVAS_SIZE, Math.round(cssHeight * scale));
	if (canvas.width !== width || canvas.height !== height) {
		canvas.width = width;
		canvas.height = height;
		try {
			renderer.resize(width, height);
		} catch (error) {
			setStatus(`Resize failed: ${errorMessage(error)}`);
		}
	}
};

const sampleAt = (values: Float64Array, offset: number): Sample => ({
	hit: (values[offset] ?? 0) > HIT_THRESHOLD,
	t: values[offset + 1] ?? Number.NaN,
	gap: values[offset + 2] ?? Number.NaN,
	u: values[offset + UV_U_OFFSET] ?? Number.NaN,
	v: values[offset + UV_V_OFFSET] ?? Number.NaN,
});

const describeSample = (name: string, sample: Sample): string => {
	let travel: string;
	if (sample.hit) {
		travel = `hit · t ${format(sample.t)}`;
	} else {
		travel = `miss · gap ${format(sample.gap)}`;
	}
	return `${name} ${travel} · UV ${format(sample.u)}, ${format(sample.v)}`;
};

const probe = (clientX: number, clientY: number): void => {
	if (!renderer) {
		return;
	}
	lastPointer = { x: clientX, y: clientY };
	const rect = canvas.getBoundingClientRect();
	if (rect.width <= 0 || rect.height <= 0) {
		return;
	}
	const x = ((clientX - rect.left) / rect.width) * canvas.width;
	const y = ((clientY - rect.top) / rect.height) * canvas.height;
	try {
		const values = renderer.probe(
			clamp(x, 0, canvas.width),
			clamp(y, 0, canvas.height),
		);
		if (values.length < MIN_PROBE_VALUES) {
			throw new Error("probe returned fewer than two samples");
		}
		const sphere = sampleAt(values, 0);
		const plane = sampleAt(values, SAMPLE_STRIDE);
		inspector.textContent = `${describeSample("Sphere", sphere)}
${describeSample("Plane", plane)}`;
	} catch (error) {
		inspector.textContent = `Probe failed: ${errorMessage(error)}`;
	}
};

const renderFrame = (): void => {
	framePending = false;
	resizeCanvas();
	if (!renderer) {
		return;
	}
	try {
		const started = performance.now();
		renderer.render(
			clamp(yaw, -YAW_LIMIT, YAW_LIMIT),
			clamp(pitch, -ANGLE_LIMIT, ANGLE_LIMIT),
			clamp(distance, radius + MIN_DISTANCE_GAP, MAX_DISTANCE),
			clamp(radius, MIN_RADIUS, MAX_RADIUS),
			mode,
		);
		cpuElapsedMs = performance.now() - started;
		renderedFrames += 1;
		setStatus("");
		if (lastPointer) {
			probe(lastPointer.x, lastPointer.y);
		}
	} catch (error) {
		setStatus(`Render failed: ${errorMessage(error)}`);
	}
};

const invalidate = (): void => {
	scheduleSave();
	if (framePending) {
		return;
	}
	framePending = true;
	window.requestAnimationFrame(renderFrame);
};

const updateMetrics = (): void => {
	if (document.visibilityState !== "visible") {
		return;
	}
	const now = performance.now();
	const elapsed = Math.max(MIN_CANVAS_SIZE, now - metricsAt);
	const frames = renderedFrames - metricsFrames;
	metricsAt = now;
	metricsFrames = renderedFrames;
	let fps = "idle";
	if (frames !== 0) {
		fps = `${((frames * MS_PER_SECOND) / elapsed).toFixed(1)}`;
	}
	let cpu = "—";
	if (cpuElapsedMs !== undefined) {
		cpu = cpuElapsedMs.toFixed(2);
	}
	let wasm: string = wasmStatus;
	if (wasmMemory !== undefined) {
		wasm = `${toMiB(wasmMemory.buffer.byteLength)} MiB`;
	}
	let heap = "unavailable";
	const browserMemory = browserPerformance.memory;
	if (browserMemory !== undefined) {
		heap = `${toMiB(browserMemory.usedJSHeapSize)} MiB`;
	}
	metrics.textContent = `FPS ${fps} · CPU ${cpu}ms · JS ${heap} / WASM ${wasm}`;
};

const startMetrics = (): void => {
	if (metricsTimer !== undefined) {
		return;
	}
	metricsAt = performance.now();
	metricsFrames = renderedFrames;
	updateMetrics();
	metricsTimer = window.setInterval(updateMetrics, METRICS_INTERVAL_MS);
};

const stopMetrics = (): void => {
	if (metricsTimer === undefined) {
		return;
	}
	window.clearInterval(metricsTimer);
	metricsTimer = undefined;
};

const zoom = (amount: number): void => {
	distance =
		radius +
		clamp(
			Math.max(distance - radius, MIN_DISTANCE_GAP) * Math.exp(amount),
			MIN_DISTANCE_GAP,
			MAX_DISTANCE - radius,
		);
	invalidate();
};

radiusInput.addEventListener("input", () => {
	radius = clamp(Number(radiusInput.value), MIN_RADIUS, MAX_RADIUS);
	distance = clamp(distance, radius + MIN_DISTANCE_GAP, MAX_DISTANCE);
	radiusOutput.value = radius.toFixed(2);
	invalidate();
});

modeSelect.addEventListener("change", () => {
	mode = readMode();
	updateControls();
	invalidate();
});

focusButton.addEventListener("click", () => {
	yaw = Math.PI / 2;
	pitch = 0;
	distance = FOCUS_DISTANCE_FACTOR * radius;
	updateControls();
	invalidate();
});

resetButton.addEventListener("click", resetView);

canvas.addEventListener("pointerdown", (event: PointerEvent) => {
	pointerId = event.pointerId;
	previousX = event.clientX;
	previousY = event.clientY;
	canvas.setPointerCapture(event.pointerId);
	canvas.focus();
});

canvas.addEventListener("pointermove", (event: PointerEvent) => {
	probe(event.clientX, event.clientY);
	if (pointerId !== event.pointerId) {
		return;
	}
	yaw -= (event.clientX - previousX) * POINTER_SCALE;
	pitch = clamp(
		pitch + (event.clientY - previousY) * POINTER_SCALE,
		-ANGLE_LIMIT,
		ANGLE_LIMIT,
	);
	previousX = event.clientX;
	previousY = event.clientY;
	invalidate();
});

const endPointer = (event: PointerEvent): void => {
	if (pointerId !== event.pointerId) {
		return;
	}
	pointerId = undefined;
	if (canvas.hasPointerCapture(event.pointerId)) {
		canvas.releasePointerCapture(event.pointerId);
	}
};

canvas.addEventListener("pointerup", endPointer);
canvas.addEventListener("pointercancel", endPointer);
canvas.addEventListener(
	"wheel",
	(event: WheelEvent) => {
		event.preventDefault();
		zoom(event.deltaY * WHEEL_SCALE);
	},
	{ passive: false },
);

canvas.addEventListener("keydown", (event: KeyboardEvent) => {
	if (event.key === "ArrowLeft") {
		yaw -= KEY_STEP;
	} else if (event.key === "ArrowRight") {
		yaw += KEY_STEP;
	} else if (event.key === "ArrowUp") {
		pitch = clamp(pitch - KEY_STEP, -ANGLE_LIMIT, ANGLE_LIMIT);
	} else if (event.key === "ArrowDown") {
		pitch = clamp(pitch + KEY_STEP, -ANGLE_LIMIT, ANGLE_LIMIT);
	} else if (event.key === "+" || event.key === "=") {
		zoom(-KEY_STEP);
	} else if (event.key === "-" || event.key === "_") {
		zoom(KEY_STEP);
	} else if (event.key.toLowerCase() === "r") {
		resetView();
	} else {
		return;
	}
	event.preventDefault();
	invalidate();
});

window.addEventListener("resize", () => {
	resizeCanvas();
	invalidate();
});

window.addEventListener("hashchange", () => {
	applyView(parseView(location.hash));
	updateControls();
	invalidate();
});

document.addEventListener("visibilitychange", () => {
	if (document.visibilityState === "visible") {
		startMetrics();
	} else {
		stopMetrics();
	}
});

metrics.title =
	"FPS is rendered frames per second; CPU submission walltime is JavaScript wall-clock around render submission, not process CPU usage or GPU time; WASM and JS memory are MiB.";
updateControls();
if (document.visibilityState === "visible") {
	startMetrics();
}

const start = async (): Promise<void> => {
	try {
		const wasm = await init();
		wasmMemory = wasm.memory;
		if (wasmMemory !== undefined) {
			wasmStatus = "available";
			canvas.dataset.wasmMemory = "available";
		} else {
			wasmStatus = "unavailable";
			canvas.dataset.wasmMemory = "unavailable";
		}
		renderer = await Renderer.create(canvas);
		resizeCanvas();
		invalidate();
	} catch (error) {
		setStatus(`WebGPU initialization failed: ${errorMessage(error)}`);
		wasmStatus = "unavailable";
	}
};

await start();
