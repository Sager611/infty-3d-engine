// biome-ignore-all assist/source/organizeImports: Bun test imports are intentionally grouped.
// biome-ignore-all lint/correctness/useImportExtensions: Bun resolves TypeScript test modules.
// biome-ignore-all lint/nursery/noUnresolvedImports: Bun provides the test module.
// biome-ignore-all lint/nursery/useMaxParams: Test fixture parameters mirror scene fields.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a Bun test, not a Qwik component.
// biome-ignore-all lint/style/noMagicNumbers: Camera contract values are asserted directly.
import { describe, expect, test } from "bun:test";
import {
	cameraBasis,
	frameCamera,
	lookCamera,
	MAX_SPEED,
	MIN_SPEED,
	moveCamera,
	orbitCamera,
	STOPPED,
	travelSpeed,
	zoomCamera,
	type Movement,
} from "./camera";
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

function movement(forward: number, side: number): Movement {
	return { forward, side, onlyForward: forward === 1 && side === 0 };
}
function eye(c: PureCamera): readonly [number, number, number] {
	return [
		c.target[0] + c.distance * Math.cos(c.pitch) * Math.sin(c.yaw),
		c.target[1] + c.distance * Math.sin(c.pitch),
		c.target[2] + c.distance * Math.cos(c.pitch) * Math.cos(c.yaw),
	];
}
function closePosition(
	actual: readonly number[],
	expected: readonly number[],
): void {
	for (let i = 0; i < 3; i += 1) {
		// biome-ignore lint/suspicious/noMisplacedAssertion: This shared numeric assertion is called only by tests.
		expect(actual[i]).toBeCloseTo(expected[i] ?? 0, 11);
	}
}
const front: PureCamera = {
	yaw: 0,
	pitch: 0,
	distance: 5,
	target: [0, 0, 0],
};
describe("free camera", () => {
	test("drag changes orientation without orbiting the eye", () => {
		const moved = lookCamera(camera, 25, -10);
		closePosition(eye(moved), eye(camera));
		expect(moved.yaw).toBe(0.2);
		expect(moved.pitch).toBe(-0.28);
		expect(moved.distance).toBe(camera.distance);
		expect(lookCamera(camera, 25, -10)).toEqual(moved);
		closePosition(eye(lookCamera(moved, -25, 10)), eye(camera));
		closePosition(cameraBasis(camera).origin, eye(camera));
	});
	test("translation is camera relative; diagonals keep base speed", () => {
		closePosition(eye(moveCamera(front, movement(1, 0), 4, 0.05)), [0, 0, 4.8]);
		closePosition(
			eye(moveCamera(front, movement(-1, 0), 4, 0.05)),
			[0, 0, 5.2],
		);
		closePosition(
			eye(moveCamera(front, movement(0, -1), 4, 0.05)),
			[-0.2, 0, 5],
		);
		closePosition(eye(moveCamera(front, movement(0, 1), 4, 0.05)), [0.2, 0, 5]);
		const diagonal = moveCamera(front, movement(1, 1), 4, 0.05);
		expect(Math.hypot(...diagonal.target)).toBeCloseTo(0.2, 12);
		closePosition(
			eye(moveCamera({ ...front, yaw: Math.PI / 2 }, movement(1, 0), 4, 0.05)),
			[4.8, 0, 0],
		);
		closePosition(
			moveCamera({ ...front, pitch: Math.PI / 6 }, movement(1, 0), 4, 0.05)
				.target,
			[0, -0.1, -Math.sqrt(3) / 10],
		);
	});
	test("stopped and zero-time steps are idempotent; pauses never catch up", () => {
		expect(moveCamera(front, STOPPED, 4, 0.05)).toBe(front);
		expect(moveCamera(front, movement(1, 0), 4, 0)).toBe(front);
		expect(moveCamera(front, movement(1, 0), 4, 10)).toEqual(
			moveCamera(front, movement(1, 0), 4, 0.05),
		);
		const half = moveCamera(front, movement(1, 0), 4, 0.025);
		closePosition(
			moveCamera(half, movement(1, 0), 4, 0.025).target,
			moveCamera(front, movement(1, 0), 4, 0.05).target,
		);
	});
});

describe("travel policy", () => {
	test("wheel scales bounded travel speed, not the pose", () => {
		expect(travelSpeed(3, -1)).toBeGreaterThan(3);
		expect(travelSpeed(3, 1)).toBeLessThan(3);
		expect(travelSpeed(3, 0)).toBe(3);
		expect(travelSpeed(3, 1e6)).toBe(MIN_SPEED);
		expect(travelSpeed(3, -1e6)).toBe(MAX_SPEED);
		for (const [speed, amount] of [
			[0, 0],
			[Number.NaN, 0],
			[3, Number.POSITIVE_INFINITY],
		]) {
			expect(() => travelSpeed(speed ?? 0, amount ?? 0)).toThrow();
		}
	});
	test("navigation crosses scene editing bounds but retains numeric bounds", () => {
		const moved = moveCamera(
			{ ...front, target: [20, 0, 0] },
			movement(0, 1),
			4,
			0.05,
		);
		expect(moved.target[0]).toBeCloseTo(20.2, 12);
		expect(() =>
			moveCamera({ ...front, target: [1024, 0, 0] }, movement(0, 1), 4, 0.05),
		).toThrow();
	});
	test("invalid numerical requests fail explicitly", () => {
		for (const seconds of [-1, Number.NaN, Number.POSITIVE_INFINITY]) {
			expect(() => moveCamera(front, movement(1, 0), 4, seconds)).toThrow();
		}
		expect(() => moveCamera(front, movement(Number.NaN, 0), 4, 0.05)).toThrow();
		expect(() => moveCamera(front, movement(1, 0), 0, 0.05)).toThrow();
		expect(() => lookCamera(front, Number.POSITIVE_INFINITY, 0)).toThrow();
	});
});
