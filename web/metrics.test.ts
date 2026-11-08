// biome-ignore-all assist/source/organizeImports: Bun test imports are intentionally grouped.
// biome-ignore-all lint/correctness/useImportExtensions: Bun resolves TypeScript test modules.
// biome-ignore-all lint/nursery/noUnresolvedImports: Bun provides the test module.
// biome-ignore-all lint/style/noMagicNumbers: Metric fixtures intentionally use byte and timing values.
import { describe, expect, test } from "bun:test";
import { formatMetrics } from "./metrics";

describe("metrics formatting", () => {
	test("reports idle when no frame arrived", () => {
		expect(
			formatMetrics({
				frames: 0,
				elapsedMs: 1000,
				cpuSubmissionMs: undefined,
				jsHeapBytes: undefined,
				wasmBytes: 0,
			}),
		).toBe("FPS idle · CPU —ms · JS unavailable / WASM 0.0 MiB");
	});

	test("reports average FPS and submission walltime", () => {
		expect(
			formatMetrics({
				frames: 30,
				elapsedMs: 1500,
				cpuSubmissionMs: 2.5,
				jsHeapBytes: 2 * 1024 * 1024,
				wasmBytes: 3 * 1024 * 1024,
			}),
		).toBe("FPS 20.0 · CPU 2.50ms · JS 2.0 MiB / WASM 3.0 MiB");
	});

	test("falls back when JS heap is not exposed and keeps byte lengths", () => {
		expect(
			formatMetrics({
				frames: 1,
				elapsedMs: 1000,
				cpuSubmissionMs: 0,
				jsHeapBytes: undefined,
				wasmBytes: 1_048_576,
			}),
		).toBe("FPS 1.0 · CPU 0.00ms · JS unavailable / WASM 1.0 MiB");
	});
});
