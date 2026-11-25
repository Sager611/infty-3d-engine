// biome-ignore-all lint/nursery/noUnresolvedImports: Bun supplies its test module.
// biome-ignore-all lint/style/noMagicNumbers: Quaternion fixtures.
import { expect, test } from "bun:test";
import {
	IDENTITY_ROTATION,
	normalizeRotation,
	type Rotation,
	rotateVector,
	turnRotation,
} from "./rotation.ts";

test("quaternions normalize finite extremes, signs and signed zero idempotently", () => {
	for (const value of [
		[0, 10, 0, 10],
		[0, 1e300, 0, 1e300],
		[0, 1e-300, 0, 1e-300],
		[0, -10, 0, -10],
		[-0, 0, -0, 1],
	] satisfies Rotation[]) {
		const normalized = normalizeRotation(value);
		expect(Math.hypot(...normalized)).toBeCloseTo(1, 14);
		expect(normalizeRotation(normalized)).toEqual(normalized);
		expect(Object.isFrozen(normalized)).toBe(true);
	}
	for (const bad of [
		[0, 0, 0, 0],
		[0, 0, 0, Number.NaN],
		[0, 0, 0, Number.POSITIVE_INFINITY],
	] satisfies Rotation[]) {
		expect(() => normalizeRotation(bad)).toThrow();
	}
	expect(normalizeRotation([0, 0, 0, -Number.MIN_VALUE])).toEqual(
		IDENTITY_ROTATION,
	);
});

test("world-axis turns compose and preserve vector lengths without rotating the pivot", () => {
	const quarter = turnRotation(undefined, 1, Math.PI / 2);
	const turned = rotateVector(quarter, [0, 0, 1]);
	expect(turned[0]).toBeCloseTo(1, 14);
	expect(turned[1]).toBeCloseTo(0, 14);
	expect(turned[2]).toBeCloseTo(0, 14);
	const full = turnRotation(quarter, 1, -Math.PI / 2);
	expect(rotateVector(full, [2, 3, 4])).toEqual([2, 3, 4]);
	const mixed = turnRotation(quarter, 0, Math.PI / 2);
	const twice = rotateVector(mixed, [0, 0, 1]);
	expect(twice[0]).toBeCloseTo(1, 14);
	expect(twice[1]).toBeCloseTo(0, 14);
	expect(Math.hypot(...rotateVector(mixed, [2, 3, 4]))).toBeCloseTo(
		Math.sqrt(29),
		13,
	);
	expect(rotateVector(mixed, [0, 0, 0])).toEqual([0, 0, 0]);
	expect(() => turnRotation(mixed, 2, Number.NaN)).toThrow();
});
