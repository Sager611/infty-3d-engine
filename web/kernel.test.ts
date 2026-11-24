// biome-ignore-all lint/style/noMagicNumbers: Binary protocol fixtures assert exact lane/tag/budget values.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual test module.
import { expect, test } from "bun:test";
import {
	closest_point,
	initSync,
	PreparedScene,
	query_ray,
	query_scene,
	trace_closest_point,
} from "../pkg/infty.js";

initSync({
	module: await Bun.file(
		new URL("../pkg/infty_bg.wasm", import.meta.url),
	).arrayBuffer(),
});
const maximumId: number = 2 ** 32 - 1;
const empty: Float64Array = new Float64Array();
const point: Float64Array = new Float64Array([3, 0, 0]);
const request: Float64Array = new Float64Array([1e-8, 1e-8, 4096]);
const ray: Float64Array = new Float64Array([0, 0, 3, 0, 0, -1]);
const camera: Float64Array = new Float64Array([0, 0, 3, 0, 0, 0]);
const screen: Float64Array = new Float64Array([8, 8, 4, 4]);

function tracePacket(data: Float64Array): Readonly<{
	result: Float64Array;
	events: readonly Float64Array[];
	omitted: number;
}> {
	const length = data[0] ?? Number.NaN;
	const count = data[1] ?? Number.NaN;
	const omitted = data[2] ?? Number.NaN;
	if (
		[length, count, omitted].some(
			(value) => !Number.isInteger(value) || value < 0,
		) ||
		count > 256 ||
		data.length !== 3 + length + 8 * count
	) {
		throw new Error("Malformed trace packet");
	}
	return {
		result: data.slice(3, 3 + length),
		omitted,
		events: Array.from({ length: count }, (_, i) =>
			data.slice(3 + length + 8 * i, 3 + length + 8 * (i + 1)),
		),
	};
}

test("WASM material packets leave prepared geometry queries and traces bit-identical", () => {
	for (const [kind, parameter] of [
		[1, 0],
		[2, 1.5],
		[3, 0],
		[4, 0],
	] as const) {
		for (const finish of [1, 2]) {
			const steel = new PreparedScene(
				new Float64Array([7, kind, 0, 0, 0, 1, parameter, 0]),
			);
			const brick = new PreparedScene(
				new Float64Array([7, kind, 0, 0, 0, 1, parameter, finish]),
			);
			try {
				expect(brick.closest_point(7, point, request)).toEqual(
					steel.closest_point(7, point, request),
				);
				for (const input of [
					ray,
					new Float64Array([0, 1, 3, 0, 0, -1]),
					new Float64Array([0, 3, 0, 0, -1, 0]),
				]) {
					for (const mode of [0, 1, 2]) {
						expect(brick.query_ray(input, mode)).toEqual(
							steel.query_ray(input, mode),
						);
					}
					expect(brick.trace_ray(input, 2, 16)).toEqual(
						steel.trace_ray(input, 2, 16),
					);
				}
			} finally {
				brick.free();
				steel.free();
			}
		}
	}
	expect(
		() => new PreparedScene(new Float64Array([7, 1, 0, 0, 0, 1, 0, 3])),
	).toThrow();
});

test("WASM finite leaf keeps shared carriers, real cutouts, bounds and result-neutral traces", (): void => {
	const leaf = new PreparedScene(
		new Float64Array([7, 5, 0, 0, 0, 0.07, 0, 2, 8, 5, 0.5, 0, 0, 0.07, 0, 0]),
	);
	const card = new PreparedScene(new Float64Array([7, 5, 0, 0, 0, 0.07, 0, 0]));
	try {
		expect(leaf.instance_count()).toBe(2);
		expect(leaf.geometry_count()).toBe(1);
		const input = new Float64Array([0, 0, 0.05, 0, 0, -1]);
		const hit = leaf.query_ray(input, 0);
		const flat = card.query_ray(input, 0);
		expect(hit).toHaveLength(24);
		expect(hit[1]).toBe(5);
		expect(hit[2]).toBe(1);
		const height = hit[8] ?? Number.NaN;
		expect(height).toBeLessThan(-0.005);
		expect(hit[3]).toBeCloseTo((flat[3] ?? Number.NaN) - height, 12);
		expect(hit.slice(12, 14)).toEqual(flat.slice(12, 14));
		const near = new Float64Array([0, 0, height * 0.5, 0, 0, -1]);
		expect(card.query_ray(near, 0)).toHaveLength(0);
		const nearHit = leaf.query_ray(near, 0);
		expect(Array.from(nearHit.slice(0, 3))).toEqual([7, 5, 1]);
		expect(nearHit[3]).toBeCloseTo(-height * 0.5, 12);
		const cutout = new Float64Array([-0.067, -0.067, 0.05, 0, 0, -1]);
		expect(leaf.query_ray(cutout, 0)).toHaveLength(0);
		expect(card.query_ray(cutout, 0)).toHaveLength(24);
		const closest = leaf.closest_point(
			7,
			new Float64Array([0, 0, 0.05]),
			new Float64Array([1e-20, 0, 1]),
		);
		expect(closest).toHaveLength(12);
		expect(closest[2]).toBe(1);
		expect(closest[9] ?? Number.NaN).toBeLessThanOrEqual(1);
		expect(closest[3] ?? Number.NaN).toBeLessThanOrEqual(
			closest[4] ?? Number.NaN,
		);
		const expected = leaf.query_ray(input, 2);
		for (const capacity of [0, 1, 16, 256]) {
			const trace = tracePacket(leaf.trace_ray(input, 2, capacity));
			expect(trace.result).toEqual(expected);
			expect(trace.events.length).toBeLessThanOrEqual(capacity);
		}
	} finally {
		leaf.free();
		card.free();
	}
});
test("WASM opt-in traces keep the same result and bounded prefix at every capacity", () => {
	const position = new Float64Array([0.3, 0.9, 1.3]);
	for (const [kind, parameter] of [
		[1, 0],
		[2, 1.5],
		[3, 0],
		[4, 0],
	] as const) {
		const normal = closest_point(kind, parameter, position, request);
		const full = tracePacket(
			trace_closest_point(kind, parameter, position, request, 256),
		);
		expect(full.result).toEqual(normal);
		expect(full.events.some((row) => row[0] === 7 && row[1] === 0)).toBe(true);
		const total = full.events.length + full.omitted;
		for (const limit of [0, 1]) {
			const trace = tracePacket(
				trace_closest_point(kind, parameter, position, request, limit),
			);
			expect(trace.result).toEqual(normal);
			expect(trace.events).toEqual(full.events.slice(0, limit));
			expect(trace.omitted + trace.events.length).toBe(total);
		}
	}
});

test("WASM ray traces retain real root brackets, repeated roots and world units", () => {
	const prepared = new PreparedScene(
		new Float64Array([42, 2, 2, -3, 4, 0.5, 1.5625, 0]),
	);
	try {
		for (const [y, distance] of [
			[-2.5, 0.75],
			[-2.25, 1],
		]) {
			const input = new Float64Array([0.5, y ?? Number.NaN, 4.5, 1, 0, 0]);
			for (const mode of [0, 1, 2]) {
				const normal = prepared.query_ray(input, mode);
				const trace = tracePacket(prepared.trace_ray(input, mode, 256));
				expect(trace.result).toEqual(normal);
				expect(trace.result[3]).toBeCloseTo(distance ?? Number.NaN, 12);
				expect(trace.events.every((row) => row[1] === 0 || row[1] === 42)).toBe(
					true,
				);
				expect(
					trace.events.some(
						(row) =>
							row[0] === 6 &&
							Math.abs((row[3] ?? Number.NaN) - (distance ?? Number.NaN)) <
								2e-12,
					),
				).toBe(true);
				if (y === -2.5) {
					const brackets = trace.events.filter((row) => row[0] === 3);
					for (const root of [-1.5, 1.5]) {
						expect(
							brackets.some(
								(row) =>
									(row[2] ?? Number.NaN) <= root &&
									root <= (row[3] ?? Number.NaN),
							),
						).toBe(true);
					}
				} else {
					expect(trace.events.some((row) => row[0] === 2)).toBe(true);
				}
			}
		}
	} finally {
		prepared.free();
	}
});

test("WASM closest traces separate local bounds from final world accuracy", () => {
	const prepared = new PreparedScene(
		new Float64Array([42, 2, 2, -3, 4, 0.5, 1.5, 0]),
	);
	try {
		const position = new Float64Array([2, -3, 4]);
		const normal = prepared.closest_point(42, position, request);
		const trace = tracePacket(
			prepared.trace_closest_point(42, position, request, 256),
		);
		expect(trace.result).toEqual(normal);
		expect(trace.events.every((row) => row[1] === 42)).toBe(true);
		const world = trace.events.find((row) => row[0] === 8);
		expect(world?.[2]).toBe(normal[3]);
		expect(world?.[3]).toBe(normal[4]);
		expect(world?.[4]).toBe(normal[8]);
		expect(
			trace.events.some(
				(row) => row[0] === 7 && (row[2] ?? 0) > (normal[4] ?? 0),
			),
		).toBe(true);
		const zero = tracePacket(
			prepared.trace_closest_point(
				42,
				position,
				new Float64Array([1e-8, 0, 0]),
				1,
			),
		);
		expect(zero.result).toEqual(
			prepared.closest_point(42, position, new Float64Array([1e-8, 0, 0])),
		);
		expect(zero.result[4]).toBe(Number.POSITIVE_INFINITY);
	} finally {
		prepared.free();
	}
});

test("WASM trace limits, IDs and modes reject coercion rather than wrapping", () => {
	const prepared = new PreparedScene(
		new Float64Array([42, 1, 0, 0, 0, 1, 0, 0]),
	);
	try {
		for (const limit of [
			-1,
			0.5,
			257,
			2 ** 32,
			Number.NaN,
			Number.POSITIVE_INFINITY,
		]) {
			expect(() => trace_closest_point(1, 0, point, request, limit)).toThrow(
				"trace",
			);
			expect(() =>
				prepared.trace_closest_point(42, point, request, limit),
			).toThrow("trace");
			expect(() => prepared.trace_ray(ray, 0, limit)).toThrow("trace");
		}
		for (const id of [0, 1.5, 2 ** 32 + 42, -1]) {
			expect(() => prepared.trace_closest_point(id, point, request, 0)).toThrow(
				"ID",
			);
		}
		expect(() => prepared.trace_closest_point(43, point, request, 0)).toThrow(
			"not in",
		);
		for (const mode of [-1, 0.5, 3, Number.NaN, Number.POSITIVE_INFINITY]) {
			expect(() => prepared.trace_ray(ray, mode, 0)).toThrow("mode");
		}
	} finally {
		prepared.free();
	}
});

test("WASM quartic hole misses retain computed bounds in every mode", () => {
	const input = new Float64Array([0, 0, -6, 0, 0, 1]);
	for (const size of [1, 2, 4]) {
		const prepared = new PreparedScene(
			new Float64Array([42, 2, 0, 0, 0, size, 1.5, 0]),
		);
		try {
			const gap = size * Math.sqrt(2 - Math.sqrt(3));
			for (const mode of [0, 1]) {
				expect(prepared.query_ray(input, mode).length).toBe(0);
				const trace = tracePacket(prepared.trace_ray(input, mode, 256));
				expect(trace.result.length).toBe(0);
				const world = trace.events.find((row) => row[0] === 6);
				expect(world?.[4]).toBeLessThanOrEqual(gap);
				expect(world?.[5]).toBeGreaterThanOrEqual(gap);
				expect(world?.[7]).toBeLessThan(2);
			}
			const inspection = prepared.query_ray(input, 2);
			expect(inspection[4]).toBeLessThanOrEqual(gap);
			expect(inspection[5]).toBeGreaterThanOrEqual(gap);
			expect(inspection[22]).toBeLessThan(2);
		} finally {
			prepared.free();
		}
	}
});

test("WASM crystalline instances retain fixed point work and ray miss bounds", () => {
	const origin = new Float64Array(3);
	const fixed = new Float64Array([1e-12, 0, 27]);
	const result = closest_point(4, 0, origin, fixed);
	expect(Array.from(result.slice(0, 5))).toEqual([0, 0, 1, 0, 0]);
	expect(result[9]).toBe(27);
	const short = closest_point(4, 0, origin, new Float64Array([1e-12, 0, 26]));
	expect(Array.from(short.slice(0, 5))).toEqual([
		1,
		2,
		0,
		0,
		Number.POSITIVE_INFINITY,
	]);
	expect(short[9]).toBe(0);
	expect(() => closest_point(4, 1, origin, fixed)).toThrow("parameter");
	const scene = new PreparedScene(new Float64Array([7, 4, 0, 1, 0, 2, 0, 0]));
	try {
		const closest = scene.closest_point(7, new Float64Array([0, 1, 0]), fixed);
		expect(Array.from(closest.slice(0, 5))).toEqual([0, 0, 1, 0, 0]);
		expect(Array.from(closest.slice(5, 8))).toEqual([0, 1, 0]);
		expect(closest[9]).toBe(27);
		const hit = scene.query_ray(new Float64Array([0, 1, -4, 0, 0, 1]), 2);
		expect(Array.from(hit.slice(0, 4))).toEqual([7, 4, 1, 4]);
		expect(hit[22]).toBeLessThan(2);
		const away = new Float64Array([-6, -5, -6, -1, -1, -1]);
		const trace = tracePacket(scene.trace_ray(away, 0, 256));
		expect(trace.result.length).toBe(0);
		const miss = scene.query_ray(away, 2);
		const world = trace.events.find((row) => row[0] === 6);
		expect(world?.[4]).toBe(miss[4]);
		expect(world?.[5]).toBe(miss[5]);
		expect(world?.[7]).toBeLessThan(2);
		expect(miss[21]).toBe(1);
		expect(miss[22]).toBeLessThan(2);
	} finally {
		scene.free();
	}
});

test("WASM quartic closest queries keep global bounds and finite work", () => {
	let lower = 0;
	let upper = Number.POSITIVE_INFINITY;
	for (const budget of [0, 1, 7, 127, 4096, 65_536]) {
		const result = closest_point(
			2,
			1.5,
			new Float64Array([0.3, 0.9, 1.3]),
			new Float64Array([1e-8, 0, budget]),
		);
		expect(result.length).toBe(12);
		expect(result[3]).toBeGreaterThanOrEqual(lower);
		expect(result[4]).toBeLessThanOrEqual(upper);
		expect(result[9]).toBeLessThanOrEqual(budget);
		if (budget === 0) {
			expect(Array.from(result.slice(0, 5))).toEqual([
				1,
				2,
				0,
				0,
				Number.POSITIVE_INFINITY,
			]);
		} else {
			expect(result[2]).toBe(1);
			expect(Array.from(result.slice(5, 9)).every(Number.isFinite)).toBe(true);
		}
		if (budget === 65_536) {
			expect(result[0]).toBe(0);
		}
		lower = result[3] ?? Number.NaN;
		upper = result[4] ?? Number.NaN;
	}
	const center = closest_point(2, 1.5, new Float64Array(3), request);
	const exact = Math.sqrt(3 - Math.sqrt(4.5));
	expect(center[1]).toBe(1);
	expect(center[3]).toBeLessThanOrEqual(exact);
	expect(center[4]).toBeGreaterThanOrEqual(exact);
});

test("WASM instance closest queries use world units and exact stable IDs", () => {
	const scene = new PreparedScene(
		new Float64Array([
			maximumId,
			1,
			2,
			-3,
			4,
			2,
			0,
			0,
			7,
			2,
			2,
			-3,
			4,
			0.5,
			1.5,
			0,
		]),
	);
	try {
		const world = new Float64Array([8, -3, 4]);
		const sphere = scene.closest_point(maximumId, world, request);
		expect(sphere[0]).toBe(0);
		expect(sphere[3]).toBeLessThanOrEqual(4);
		expect(sphere[4]).toBeGreaterThanOrEqual(4);
		expect(Array.from(sphere.slice(5, 8))).toEqual([4, -3, 4]);
		const quartic = scene.closest_point(
			7,
			new Float64Array([2, -3, 4]),
			request,
		);
		const exact = 0.5 * Math.sqrt(3 - Math.sqrt(4.5));
		expect(quartic[3]).toBeLessThanOrEqual(exact);
		expect(quartic[4]).toBeGreaterThanOrEqual(exact);
		for (const id of [
			0,
			7.5,
			2 ** 32 + 7,
			Number.NaN,
			Number.POSITIVE_INFINITY,
		]) {
			expect(() => scene.closest_point(id, world, request)).toThrow(
				"invalid object ID",
			);
		}
		expect(() => scene.closest_point(99, world, request)).toThrow(
			"object ID is not in this scene",
		);
		expect(scene.instance_count()).toBe(2);
	} finally {
		scene.free();
	}
});

test("WASM closest queries reject malformed accuracy and point packets", () => {
	expect(() => closest_point(2, 1.5, point.slice(0, 2), request)).toThrow(
		"point packet",
	);
	expect(() => closest_point(2, 1.5, point, request.slice(0, 2))).toThrow(
		"request packet",
	);
	for (const accuracy of [
		[0, 0, 10],
		[-1, 0, 10],
		[1e-8, 2, 10],
		[Number.NaN, 0, 10],
		[1e-8, 0, 1.5],
		[1e-8, 0, 65_537],
		[1e-8, 0, Number.POSITIVE_INFINITY],
	]) {
		expect(() =>
			closest_point(2, 1.5, point, new Float64Array(accuracy)),
		).toThrow();
	}
	expect(() =>
		closest_point(2, 1.5, new Float64Array([Number.NaN, 0, 0]), request),
	).toThrow();
});

test("WASM rejects fractional and wrapped geometry tags", () => {
	for (const kind of [
		1.5,
		3.5,
		2 ** 32 + 1,
		Number.NaN,
		Number.POSITIVE_INFINITY,
	]) {
		expect(() => closest_point(kind, 0, point, request)).toThrow(
			"invalid geometry kind",
		);
	}
});

test("WASM query modes reject fractional and wrapped numbers", () => {
	const scene = new PreparedScene(empty);
	try {
		for (const mode of [
			0.5,
			1.5,
			2.5,
			2 ** 32,
			Number.NaN,
			Number.POSITIVE_INFINITY,
		]) {
			for (const query of [
				(): Float64Array => query_ray(empty, ray, mode),
				(): Float64Array => query_scene(empty, camera, screen, mode),
				(): Float64Array => scene.query_ray(ray, mode),
				(): Float64Array => scene.query_camera(camera, screen, mode),
			]) {
				expect(query).toThrow("query mode must be 0, 1, or 2");
			}
		}
	} finally {
		scene.free();
	}
});
