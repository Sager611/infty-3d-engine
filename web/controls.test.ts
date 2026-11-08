// biome-ignore-all lint/style/noMagicNumbers: Scene fixtures use canonical contract values.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import { fieldsForObject, propertyPatch } from "./controls.ts";
import { DEFAULT_SCENE, type SceneObject } from "./scene.ts";

const sphere: SceneObject | undefined = DEFAULT_SCENE[0];
const goursat: SceneObject | undefined = DEFAULT_SCENE[1];
const plane: SceneObject | undefined = DEFAULT_SCENE[2];

if (sphere === undefined || goursat === undefined || plane === undefined) {
	throw new Error("default scene fixture is incomplete");
}

describe("property controls", () => {
	test("maps common and shape-specific fields from descriptors", () => {
		expect(fieldsForObject(sphere)).toEqual([
			{ key: "x", label: "X", value: -1.65, min: -20, max: 20, step: 0.01 },
			{ key: "y", label: "Y", value: 1, min: -20, max: 20, step: 0.01 },
			{ key: "z", label: "Z", value: 0, min: -20, max: 20, step: 0.01 },
			{ key: "size", label: "Radius", value: 1, min: 0.01, max: 4, step: 0.01 },
		]);
		expect(fieldsForObject(goursat)).toContainEqual({
			key: "parameter",
			label: "Opening β",
			value: 1.5,
			min: 1.05,
			max: 1.95,
			step: 0.01,
		});
		expect(fieldsForObject(plane)).toContainEqual({
			key: "size",
			label: "Grid size",
			value: 1,
			min: 0.01,
			max: 4,
			step: 0.01,
		});
	});

	test("returns stable immutable fields", () => {
		const fields = fieldsForObject(sphere);
		expect(Object.isFrozen(fields)).toBe(true);
		expect(fields).toEqual(fieldsForObject(sphere));
	});

	test("creates immutable position patches without mutating the object", () => {
		const object = sphere;
		const patch = propertyPatch(object, "x", 2);
		expect(patch).toEqual({ position: [2, 1, 0] });
		expect(patch.position).not.toBe(object.position);
		expect(object.position).toEqual([-1.65, 1, 0]);
	});

	test("creates size and parameter patches idempotently", () => {
		const object = goursat;
		expect(propertyPatch(object, "size", 1)).toEqual({ size: 1 });
		expect(propertyPatch(object, "parameter", 1.5)).toEqual(
			propertyPatch(object, "parameter", 1.5),
		);
	});

	test("rejects unknown and nonfinite property values", () => {
		const object = sphere;
		expect(() => propertyPatch(object, "radius", 1)).toThrow(
			"unknown property",
		);
		expect(() => propertyPatch(object, "x", Number.NaN)).toThrow("nonfinite");
		expect(() =>
			propertyPatch(object, "size", Number.POSITIVE_INFINITY),
		).toThrow("nonfinite");
	});
});
