// biome-ignore-all lint/style/noMagicNumbers: Independent scene-count, geometry and projection fixtures.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual test module.
import { expect, test } from "bun:test";
import { cameraBasis } from "./camera.ts";
import { presetSnapshot } from "./presets.ts";
import { DEFAULT_SCENE, packScene, unpackScene } from "./scene.ts";
import { DEFAULT_SNAPSHOT, parseSnapshot, serializeSnapshot } from "./state.ts";

test("showcase generation is deterministic, owned, immutable and has 4,097 valid instances", () => {
	const before = serializeSnapshot(DEFAULT_SNAPSHOT);
	const first = presetSnapshot("showcase", 1.5, 2);
	const second = presetSnapshot("showcase", 1.5, 2);
	expect(first.objects.length).toBe(4097);
	expect(first).toEqual(second);
	expect(first.objects).not.toBe(second.objects);
	expect(new Set(first.objects.map((object) => object.id)).size).toBe(4097);
	expect(
		first.objects.filter((object) => object.kind === "sphere").length,
	).toBe(2048);
	expect(
		first.objects.filter((object) => object.kind === "goursat").length,
	).toBe(2048);
	expect(first.objects.filter((object) => object.kind === "plane").length).toBe(
		1,
	);
	expect(Object.isFrozen(first) && Object.isFrozen(first.objects)).toBe(true);
	expect(
		first.objects.every(
			(object) => Object.isFrozen(object) && Object.isFrozen(object.position),
		),
	).toBe(true);
	expect(unpackScene(packScene(first.objects))).toEqual(first.objects);
	expect(first.mode).toBe(2);
	expect(serializeSnapshot(DEFAULT_SNAPSHOT)).toBe(before);
});

test("default loading reuses defaults while showcase URLs preserve every editable instance", () => {
	const normal = presetSnapshot("default", 1.5, 1);
	expect(normal.objects).toBe(DEFAULT_SCENE);
	expect(normal.mode).toBe(1);
	const showcase = presetSnapshot("showcase", 390 / 844, 0);
	expect(parseSnapshot(serializeSnapshot(showcase))).toEqual(showcase);
});

test("preset framing contains finite geometry on desktop, portrait and landscape", () => {
	for (const aspect of [1200 / 800, 390 / 844, 844 / 390]) {
		const state = presetSnapshot("showcase", aspect, 0);
		const basis = cameraBasis(state.camera);
		const up: readonly [number, number, number] = [
			basis.right[1] * basis.forward[2] - basis.right[2] * basis.forward[1],
			basis.right[2] * basis.forward[0] - basis.right[0] * basis.forward[2],
			basis.right[0] * basis.forward[1] - basis.right[1] * basis.forward[0],
		];
		let horizontal = 0;
		let vertical = 0;
		let nearest = Number.POSITIVE_INFINITY;
		for (const object of state.objects.filter(
			(item) => item.kind !== "plane",
		)) {
			let radius = object.size;
			if (object.kind === "goursat") {
				radius *= Math.sqrt(3 + Math.sqrt(3 * object.parameter));
			}
			const offset = object.position.map(
				(value, i) => value - (basis.origin[i] ?? Number.NaN),
			);
			const x = offset.reduce(
				(sum, value, i) => sum + value * (basis.right[i] ?? Number.NaN),
				0,
			);
			const y = offset.reduce(
				(sum, value, i) => sum + value * (up[i] ?? Number.NaN),
				0,
			);
			const z =
				offset.reduce(
					(sum, value, i) => sum + value * (basis.forward[i] ?? Number.NaN),
					0,
				) - radius;
			nearest = Math.min(nearest, z);
			horizontal = Math.max(
				horizontal,
				(1.8 * (Math.abs(x) + radius)) / (z * 0.5 * aspect),
			);
			vertical = Math.max(vertical, (1.8 * (Math.abs(y) + radius)) / (z * 0.5));
		}
		expect(nearest).toBeGreaterThan(0);
		expect(horizontal).toBeLessThanOrEqual(1);
		expect(vertical).toBeLessThanOrEqual(1);
	}
});

test("Brick relief preset opens a framed material preview and round-trips", () => {
	const state = presetSnapshot("brick", 1265 / 791, 0);
	const panel = state.objects[0];
	if (panel === undefined) {
		throw new Error("missing brick panel");
	}
	expect(state.mode).toBe(3);
	expect(panel).toMatchObject({ kind: "finite", material: "brick" });
	expect(state.camera.target).toEqual(panel.position);
	expect(Math.abs(state.camera.yaw)).toBeGreaterThan(0.2);
	expect(parseSnapshot(serializeSnapshot(state))).toEqual(state);
});

test("unknown presets and invalid viewport geometry never produce a scene", () => {
	expect(() => presetSnapshot("missing", 1, 0)).toThrow("preset");
	for (const aspect of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
		expect(() => presetSnapshot("showcase", aspect, 0)).toThrow("aspect");
	}
});
