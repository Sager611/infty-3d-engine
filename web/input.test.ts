// biome-ignore-all lint/style/noMagicNumbers: Input contract values are asserted directly.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import { orbitCamera } from "./camera.ts";
import { isClick } from "./input.ts";
import type { PureCamera } from "./state.ts";

const camera: PureCamera = {
	yaw: 0.4,
	pitch: -0.2,
	distance: 5,
	target: [0, 1, 0],
};

describe("input click threshold", () => {
	test("accepts only finite movement shorter than four pixels", () => {
		expect(isClick(0, 0)).toBe(true);
		expect(isClick(3.9, 0)).toBe(true);
		expect(isClick(4, 0)).toBe(false);
		expect(isClick(2.4, 3.2)).toBe(false);
	});

	test("rejects invalid movement", () => {
		expect(isClick(Number.NaN, 0)).toBe(false);
		expect(isClick(0, Number.POSITIVE_INFINITY)).toBe(false);
	});
});

describe("absolute orbit integration", () => {
	test("applies total drag from the start camera", () => {
		const moved = orbitCamera(camera, 25, -10);
		expect(moved).toEqual({
			...camera,
			yaw: 0.2,
			pitch: -0.28,
		});
		expect(orbitCamera(camera, 25, -10)).toEqual(moved);
		expect(orbitCamera(moved, -25, 10)).toEqual(camera);
	});
});
