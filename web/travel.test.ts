// biome-ignore-all lint/style/noMagicNumbers: Navigation policy values are independent test cases.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { MIN_SPEED, type Movement, stepSpeed } from "./camera.ts";

const FORWARD: Movement = { forward: 1, side: 0, onlyForward: true };
function unusedHit(): never {
	throw new Error("unnecessary center query");
}

test("only sole W queries a surface; reverse, strafe, diagonals and cancelling chords keep base speed", () => {
	const movements: readonly Movement[] = [
		{ forward: 0, side: 0, onlyForward: false },
		{ forward: -1, side: 0, onlyForward: false },
		{ forward: 0, side: 1, onlyForward: false },
		{ forward: 0, side: -1, onlyForward: false },
		{ forward: 1, side: 1, onlyForward: false },
		{ forward: 1, side: -1, onlyForward: false },
		{ forward: -1, side: 1, onlyForward: false },
		{ forward: 1, side: 0, onlyForward: false },
		{ forward: -1, side: 0, onlyForward: true },
		{ forward: 1, side: 1, onlyForward: true },
	];
	for (const movement of movements) {
		expect(stepSpeed(movement, 3, 0.02, unusedHit)).toBe(3);
	}
	expect(stepSpeed(FORWARD, 3, 0, unusedHit)).toBe(3);
});

test("approach rates stay positive, monotone and bounded, without clamping motion to the surface", () => {
	for (const speed of [MIN_SPEED, 0.03, 1, 3, 10, 100]) {
		let previous = MIN_SPEED;
		for (const distance of [
			0,
			0.001,
			0.01,
			0.2,
			0.5,
			1,
			2,
			6,
			Number.MAX_VALUE,
		]) {
			const rate = stepSpeed(FORWARD, speed, 0.05, () => distance);
			expect(rate).toBeGreaterThanOrEqual(previous);
			expect(rate).toBeLessThanOrEqual(speed);
			previous = rate;
		}
		expect(stepSpeed(FORWARD, speed, 0.05, () => 0)).toBe(MIN_SPEED);
		expect(stepSpeed(FORWARD, speed, 0.05, () => Number.MAX_VALUE)).toBe(speed);
	}
	const gap = 0.000_001;
	expect(stepSpeed(FORWARD, 3, 0.05, () => gap) * 0.05).toBeGreaterThan(gap);
	let previous = MIN_SPEED;
	for (const speed of [MIN_SPEED, 0.03, 1, 3, 10, 100]) {
		const rate = stepSpeed(FORWARD, speed, 0.05, () => 0.01);
		expect(rate).toBeGreaterThanOrEqual(previous);
		previous = rate;
	}
});

test("closed-form easing cannot skip its band at high speed and agrees across time partitions", () => {
	const speed = 3;
	const gap = 0.2;
	const gain = speed - MIN_SPEED;
	const full = stepSpeed(FORWARD, speed, 0.04, () => gap) * 0.04;
	const oracle = (gap + MIN_SPEED / gain) * (1 - Math.exp(-gain * 0.04));
	expect(Math.abs(full - oracle)).toBeLessThan(1e-12);
	const first = stepSpeed(FORWARD, speed, 0.02, () => gap) * 0.02;
	const second = stepSpeed(FORWARD, speed, 0.02, () => gap - first) * 0.02;
	expect(Math.abs(first + second - full)).toBeLessThan(1e-12);
	const fast = stepSpeed(FORWARD, 100, 0.05, () => 1.25) * 0.05;
	const highGain = 100 - MIN_SPEED;
	const highOracle =
		0.25 + (1 + MIN_SPEED / highGain) * (1 - Math.exp(-highGain * 0.0475));
	expect(Math.abs(fast - highOracle)).toBeLessThan(1e-12);
	expect(fast).toBeLessThan(1.25);
	expect(stepSpeed(FORWARD, 3, 1000, () => gap)).toBe(
		stepSpeed(FORWARD, 3, 0.05, () => gap),
	);
});

test("one analytic query per active W step; misses keep speed and unresolved or invalid requests throw", () => {
	let calls = 0;
	function hit(): number {
		calls += 1;
		return 0.5;
	}
	expect(stepSpeed(FORWARD, 3, 0.02, hit)).toBeLessThan(3);
	expect(calls).toBe(1);
	expect(stepSpeed(FORWARD, 3, 0.02, () => null)).toBe(3);
	function unresolved(): never {
		throw new Error("unresolved center query");
	}
	expect(() => stepSpeed(FORWARD, 3, 0.02, unresolved)).toThrow(
		"unresolved center query",
	);
	for (const distance of [Number.NaN, Number.POSITIVE_INFINITY, -1]) {
		expect(() => stepSpeed(FORWARD, 3, 0.02, () => distance)).toThrow();
	}
	for (const seconds of [Number.NaN, Number.POSITIVE_INFINITY, -1]) {
		expect(() => stepSpeed(FORWARD, 3, seconds, hit)).toThrow();
	}
	for (const speed of [0, MIN_SPEED / 2, 101, Number.NaN]) {
		expect(() => stepSpeed(FORWARD, speed, 0.02, hit)).toThrow();
	}
	expect(() =>
		stepSpeed({ ...FORWARD, side: Number.NaN }, 3, 0.02, hit),
	).toThrow();
});
