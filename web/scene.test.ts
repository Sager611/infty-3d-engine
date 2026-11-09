// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Tests are not Qwik components.
// biome-ignore-all lint/style/noMagicNumbers: Protocol fixtures use exact contract values.
// biome-ignore-all lint/complexity/noExcessiveLinesPerFunction: The six contract cases stay together.
// biome-ignore-all lint/suspicious/noMisplacedAssertion: The side-by-side helper is test-only.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import {
	DEFAULT_SCENE,
	DESCRIPTORS,
	packScene,
	type Scene,
	unpackScene,
	updateObject,
} from "./scene.ts";

const sideBySide = (left: Scene, right: Scene): void => {
	expect(right).toEqual(left);
	expect(packScene(right)).toEqual(packScene(left));
};

describe("scene model", () => {
	test("round-trips the default scene and keeps its locked values", () => {
		const packet = packScene(DEFAULT_SCENE);
		expect(packet).toHaveLength(24);
		const unpacked = unpackScene(packet);
		sideBySide(DEFAULT_SCENE, unpacked);
		expect(DEFAULT_SCENE).toEqual([
			{
				id: 1,
				kind: "sphere",
				position: [-1.65, 1, 0],
				size: 1,
				parameter: 0,
			},
			{
				id: 2,
				kind: "goursat",
				position: [1.65, 0.75 * Math.sqrt(1 + Math.sqrt(1.5)), 0],
				size: 0.75,
				parameter: 1.5,
			},
			{
				id: 3,
				kind: "plane",
				position: [0, 0, 0],
				size: 1,
				parameter: 0,
			},
		]);
	});

	test("exposes typed bounds for all supported kinds", () => {
		expect(Object.keys(DESCRIPTORS)).toEqual([
			"sphere",
			"goursat",
			"plane",
			"crystal",
		]);
		expect(DESCRIPTORS.sphere.size).toEqual({ min: 0.01, max: 4 });
		expect(DESCRIPTORS.goursat.parameter).toEqual({
			min: 1.05,
			max: 1.95,
		});
		expect(DESCRIPTORS.plane.parameter).toEqual({ min: 0, max: 0 });
	});

	test("clones caller position arrays without freezing or changing them", () => {
		const callerPosition: [number, number, number] = [2, 2, 2];
		const changed = updateObject(DEFAULT_SCENE, 2, {
			position: callerPosition,
		});
		callerPosition[0] = 99;
		expect(Object.isFrozen(callerPosition)).toBe(false);
		expect(callerPosition).toEqual([99, 2, 2]);
		expect(changed[1]?.position).toEqual([2, 2, 2]);
		expect(changed[1]?.position).not.toBe(callerPosition);
	});

	test("rejects sparse caller positions", () => {
		const sparsePosition = [2, 2, 2] satisfies [number, number, number];
		Reflect.deleteProperty(sparsePosition, "1");
		expect(() =>
			updateObject(DEFAULT_SCENE, 2, { position: sparsePosition }),
		).toThrow("nonfinite");
	});

	test("rejects malformed packet lengths, values, kinds, and materials", () => {
		expect(() => unpackScene(new Float64Array(7))).toThrow("invalid length");
		expect(() =>
			unpackScene(new Float64Array([1, 1, 0, 0, Number.NaN, 1, 0, 0])),
		).toThrow("nonfinite");
		expect(() =>
			unpackScene(new Float64Array([1.5, 1, 0, 0, 0, 1, 0, 0])),
		).toThrow("invalid id");
		expect(() =>
			unpackScene(new Float64Array([1, 9, 0, 0, 0, 1, 0, 0])),
		).toThrow("invalid kind");
		expect(() =>
			unpackScene(new Float64Array([1, 1, 0, 0, 0, 0, 0, 0])),
		).toThrow("invalid size");
		expect(() =>
			unpackScene(new Float64Array([1, 1, 0, 0, 0, 1, 1, 0])),
		).toThrow("invalid parameter");
		expect(() =>
			unpackScene(new Float64Array([1, 1, 0, 0, 0, 1, 0, 2])),
		).toThrow("invalid material");
		expect(() =>
			unpackScene(
				new Float64Array([1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0]),
			),
		).toThrow("duplicate");
	});

	test("keeps descriptor constraints narrow", () => {
		expect(() =>
			packScene([
				{
					id: 1,
					kind: "goursat",
					position: [0, 0, 0],
					size: 1,
					parameter: 1,
				},
			]),
		).toThrow("invalid parameter");
	});

	test("does not mutate and repeated absolute updates are idempotent", () => {
		const original = structuredClone(DEFAULT_SCENE);
		const changed = updateObject(DEFAULT_SCENE, 2, {
			position: [2, 2, 2],
			size: 1,
			parameter: 1.75,
		});
		expect(DEFAULT_SCENE).toEqual(original);
		expect(
			updateObject(DEFAULT_SCENE, 2, {
				position: [2, 2, 2],
				size: 1,
				parameter: 1.75,
			}),
		).toEqual(changed);
		expect(changed).not.toBe(DEFAULT_SCENE);
		expect(changed[1]).not.toBe(DEFAULT_SCENE[1]);
	});

	test("rejects missing IDs and invalid update values", () => {
		expect(() => updateObject(DEFAULT_SCENE, 99, {})).toThrow("unknown id");
		expect(() =>
			updateObject(DEFAULT_SCENE, 1, { position: [21, 0, 0] }),
		).toThrow("position");
		expect(() => updateObject(DEFAULT_SCENE, 1, { size: 0 })).toThrow("size");
		expect(() => updateObject(DEFAULT_SCENE, 2, { parameter: 2 })).toThrow(
			"parameter",
		);
	});
});

test("thousands of mixed instances survive packets and absolute updates", () => {
	const objects: Scene = Array.from({ length: 4096 }, (_, index) => {
		let kind: "sphere" | "goursat" = "sphere";
		let parameter = 0;
		if (index % 2 !== 0) {
			kind = "goursat";
			parameter = 1.5;
		}
		return { id: index + 1, kind, position: [0, 0, 0], size: 0.1, parameter };
	});
	sideBySide(objects, unpackScene(packScene(objects)));
	expect(updateObject(objects, 4096, { size: 0.2 })[4095]?.size).toBe(0.2);
});
