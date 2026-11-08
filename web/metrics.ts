// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a plain browser adapter, not Qwik.
const browserPerformance: Performance & {
	readonly memory?: Readonly<Record<"usedJSHeapSize", number>>;
} = performance;
const MS_PER_SECOND: number = Number("1000");
const BYTES_PER_MIB: number = Number("1048576");
const METRICS_INTERVAL_MS: number = Number("1000");

interface MetricsState {
	frames: number;
	elapsedMs: number;
	cpuSubmissionMs: number | undefined;
	jsHeapBytes: number | undefined;
	wasmBytes: number | undefined;
}

type Meter = Readonly<{
	record: (elapsedMilliseconds: number) => void;
	dispose: () => void;
}>;

const toMiB = (bytes: number): string => (bytes / BYTES_PER_MIB).toFixed(1);

const formatMetrics = (state: Readonly<MetricsState>): string => {
	let fps = "idle";
	if (state.frames !== 0) {
		fps = (
			(state.frames * MS_PER_SECOND) /
			Math.max(1, state.elapsedMs)
		).toFixed(1);
	}
	let cpu = "—";
	if (state.cpuSubmissionMs !== undefined) {
		cpu = state.cpuSubmissionMs.toFixed(2);
	}
	let js = "unavailable";
	if (state.jsHeapBytes !== undefined) {
		js = `${toMiB(state.jsHeapBytes)} MiB`;
	}
	let wasm = "unavailable";
	if (state.wasmBytes !== undefined) {
		wasm = `${toMiB(state.wasmBytes)} MiB`;
	}
	return `FPS ${fps} · CPU ${cpu}ms · JS ${js} / WASM ${wasm}`;
};

// biome-ignore lint/complexity/noExcessiveLinesPerFunction: Timer lifecycle stays local to this adapter.
const startMetrics = (
	element: HTMLElement,
	memory: WebAssembly.Memory | undefined,
): Meter => {
	let metricsState: MetricsState = {
		frames: 0,
		elapsedMs: 1,
		cpuSubmissionMs: undefined,
		jsHeapBytes: undefined,
		wasmBytes: undefined,
	};
	let previousFrames = 0;
	let metricsAt = browserPerformance.now();
	let metricsTimer: number | undefined;
	let disposed = false;

	const stopTimer = (): void => {
		if (metricsTimer === undefined) {
			return;
		}
		window.clearInterval(metricsTimer);
		metricsTimer = undefined;
	};

	const tick = (): void => {
		if (document.visibilityState !== "visible") {
			return;
		}
		const now = browserPerformance.now();
		const elapsedMs = Math.max(1, now - metricsAt);
		const frames = metricsState.frames - previousFrames;
		metricsState = {
			...metricsState,
			elapsedMs,
			jsHeapBytes: browserPerformance.memory?.usedJSHeapSize,
			wasmBytes: memory?.buffer.byteLength,
		};
		previousFrames = metricsState.frames;
		metricsAt = now;
		element.textContent = formatMetrics({ ...metricsState, frames });
	};

	const startTimer = (): void => {
		if (
			disposed ||
			metricsTimer !== undefined ||
			document.visibilityState !== "visible"
		) {
			return;
		}
		metricsAt = browserPerformance.now();
		previousFrames = metricsState.frames;
		tick();
		metricsTimer = window.setInterval(tick, METRICS_INTERVAL_MS);
	};

	const visibilityChanged = (): void => {
		if (document.visibilityState === "visible") {
			startTimer();
			return;
		}
		stopTimer();
	};

	const record = (elapsedMilliseconds: number): void => {
		if (disposed) {
			return;
		}
		metricsState = {
			...metricsState,
			frames: metricsState.frames + 1,
			cpuSubmissionMs: elapsedMilliseconds,
		};
	};

	const dispose = (): void => {
		if (disposed) {
			return;
		}
		disposed = true;
		stopTimer();
		document.removeEventListener("visibilitychange", visibilityChanged);
	};

	document.addEventListener("visibilitychange", visibilityChanged);
	element.title =
		"FPS is rendered frames per second; CPU submission walltime is JavaScript wall-clock around render submission, not process CPU usage or GPU time; WASM and JS memory are MiB.";
	startTimer();

	return { record, dispose };
};

export { formatMetrics, startMetrics };
export type { Meter };
