// biome-ignore-all lint/style/noMagicNumbers: Binary protocol fixtures assert exact lane/tag/budget values.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual test module.
import { expect, test } from "bun:test";
import {
	closest_point,
	initSync,
	PreparedScene,
	query_ray,
	query_scene,
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
