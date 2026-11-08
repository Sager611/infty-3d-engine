// biome-ignore-all assist/source/organizeImports: Bun test imports are intentionally grouped.
// biome-ignore-all lint/correctness/useImportExtensions: Bun resolves TypeScript test modules.
// biome-ignore-all lint/nursery/noUnresolvedImports: Bun provides the test module.
// biome-ignore-all lint/nursery/useMaxParams: Test fixture parameters mirror scene fields.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a Bun test, not a Qwik component.
// biome-ignore-all lint/style/noMagicNumbers: Camera contract values are asserted directly.
import { describe, expect, test } from "bun:test";
import { frameCamera, orbitCamera, zoomCamera } from "./camera";
import type { PureCamera } from "./state";
import type { SceneObject } from "./scene";

const camera: PureCamera = {
	yaw: 0.4,
	pitch: -0.2,
	distance: 5,
	target: [0, 1, 0],
};

const sphere = (
	id: number,
	x: number,
	y: number,
	z: number,
	size: number,
): SceneObject => ({
	id,
	kind: "sphere",
	position: [x, y, z],
	size,
	parameter: 0,
});

describe("camera gestures", () => {
	test("uses total drag from the start and reverses exactly", () => {
		const moved = orbitCamera(camera, 25, -10);
		expect(moved).toEqual({
			...camera,
			yaw: 0.2,
			pitch: -0.28,
		});
		expect(orbitCamera(moved, -25, 10)).toEqual(camera);
		expect(orbitCamera(camera, 25, -10)).toEqual(moved);
	});

	test("zooms around the surface hit gap", () => {
		expect(
			zoomCamera({ ...camera, distance: 5 }, Math.log(0.5), 4).distance,
		).toBe(3);
		expect(zoomCamera({ ...camera, distance: 5 }, -100, 0).distance).toBe(5);
		expect(() => zoomCamera(camera, Number.NaN, null)).toThrow();
		expect(() => zoomCamera(camera, 0, -1)).toThrow();
		expect(() => zoomCamera(camera, 0, Number.POSITIVE_INFINITY)).toThrow();
	});
});

describe("camera framing", () => {
	test("frames both objects in portrait aspect", () => {
		const objects = [sphere(1, -2, 0, 0, 1), sphere(2, 2, 0, 0, 1)];
		const portrait = frameCamera(camera, objects, [], 0.5);
		const landscape = frameCamera(camera, objects, [], 2);
		expect(portrait.target).toEqual([0, 0, 0]);
		expect(portrait.yaw).toBe(camera.yaw);
		expect(portrait.pitch).toBe(camera.pitch);
		expect(portrait.distance).toBeGreaterThan(landscape.distance);
	});

	test("frames a plane at its origin and leaves an empty scene alone", () => {
		const plane: SceneObject = {
			id: 3,
			kind: "plane",
			position: [2, 3, 4],
			size: 1,
			parameter: 0,
		};
		expect(frameCamera(camera, [plane], [], 1).target).toEqual([2, 3, 4]);
		expect(frameCamera(camera, [], [], 1)).toEqual(camera);
	});

	test("frames a plane when its unused bounds are nonfinite", () => {
		const plane: SceneObject = {
			id: 3,
			kind: "plane",
			position: [2, 3, 4],
			size: Number.NaN,
			parameter: Number.POSITIVE_INFINITY,
		};
		expect(frameCamera(camera, [plane], [], 1).target).toEqual([2, 3, 4]);
	});

	test("rejects invalid aspect and unknown selected object", () => {
		expect(() => frameCamera(camera, [], [], Number.NaN)).toThrow();
		expect(() => frameCamera(camera, [], [], 101)).toThrow();
		expect(() => frameCamera(camera, [], [7], 1)).toThrow();
	});
});
