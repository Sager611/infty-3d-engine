// biome-ignore-all lint/style/noMagicNumbers: Navigation policy values are independent test cases.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { MIN_SPEED, type Movement, stepSpeed } from "./camera.ts";

const FORWARD: Movement = { forward: 1, side: 0 };
function unusedHit(): never {
	throw new Error("unnecessary center query");
}
// Exact gap covered in t seconds of v = speed·x·(1/12 + 11/12·x), x = gap/0.25.
function travelled(speed: number, gap: number, t: number): number {
	const a = speed / 3;
	const b = (speed * 11) / 0.75;
	const decay = Math.exp(-a * t);
	return gap - (a * gap * decay) / (a + b * gap * (1 - decay));
}

test("close up, travel covers about one gap per second at the initial speed", () => {
	for (const gap of [0.000_01, 0.001, 0.01]) {
		const rate = stepSpeed({ forward: 0, side: 1 }, 3, 0.02, () => gap) / gap;
		expect(rate).toBeGreaterThan(0.99);
		expect(rate).toBeLessThan(1.5);
	}
	expect(stepSpeed({ forward: 0, side: 1 }, 3, 0.02, () => 0.25)).toBe(3);
});

test("every direction slows near the looked-at geometry; idle and misses keep base speed", () => {
	expect(stepSpeed({ forward: 0, side: 0 }, 3, 0.02, unusedHit)).toBe(3);
	expect(stepSpeed(FORWARD, 3, 0, unusedHit)).toBe(3);
	for (const movement of [
		{ forward: -1, side: 0 },
		{ forward: 0, side: 1 },
		{ forward: 0, side: -1 },
		{ forward: -1, side: 1 },
	]) {
		expect(stepSpeed(movement, 3, 0.02, () => 0.125)).toBeCloseTo(0.8125, 12);
		expect(stepSpeed(movement, 3, 0.02, () => null)).toBe(3);
		expect(stepSpeed(movement, 3, 0.02, () => 1)).toBe(3);
	}
	for (const side of [1, -1]) {
		const diagonal = { forward: 1, side };
		const rate = stepSpeed(diagonal, 3, 0.04, () => 0.2);
		expect(rate).toBeLessThan(3);
		expect(rate).toBeGreaterThan(stepSpeed(FORWARD, 3, 0.04, () => 0.2));
		const first = stepSpeed(diagonal, 3, 0.02, () => 0.2);
		const nearer = 0.2 - Math.SQRT1_2 * first * 0.02;
		const second = stepSpeed(diagonal, 3, 0.02, () => nearer);
		expect(Math.abs((first + second) * 0.02 - rate * 0.04)).toBeLessThan(1e-12);
	}
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
	const gap = 0.000_000_001;
	expect(stepSpeed(FORWARD, 3, 0.05, () => gap) * 0.05).toBeGreaterThan(gap);
	let previous = MIN_SPEED;
	for (const speed of [MIN_SPEED, 0.03, 1, 3, 10, 100]) {
		const rate = stepSpeed(FORWARD, speed, 0.05, () => 0.01);
		expect(rate).toBeGreaterThanOrEqual(previous);
		previous = rate;
	}
});

test("micro-unit inspection follows the exact easing without raising the positive floor", () => {
	const distance = stepSpeed(FORWARD, 3, 0.05, () => 0.000_01) * 0.05;
	expect(Math.abs(distance - travelled(3, 0.000_01, 0.05))).toBeLessThan(1e-18);
	expect(stepSpeed(FORWARD, 3, 0.05, () => 0)).toBe(MIN_SPEED);
});

test("closed-form easing cannot skip its band at high speed and agrees across time partitions", () => {
	const speed = 3;
	const gap = 0.2;
	const full = stepSpeed(FORWARD, speed, 0.04, () => gap) * 0.04;
	expect(Math.abs(full - travelled(speed, gap, 0.04))).toBeLessThan(1e-12);
	const first = stepSpeed(FORWARD, speed, 0.02, () => gap) * 0.02;
	const second = stepSpeed(FORWARD, speed, 0.02, () => gap - first) * 0.02;
	expect(Math.abs(first + second - full)).toBeLessThan(1e-12);
	const fast = stepSpeed(FORWARD, 100, 0.05, () => 1.25) * 0.05;
	expect(Math.abs(fast - 1 - travelled(100, 0.25, 0.04))).toBeLessThan(1e-12);
	expect(fast).toBeLessThan(1.25);
	expect(stepSpeed(FORWARD, 3, 1000, () => gap)).toBe(
		stepSpeed(FORWARD, 3, 0.05, () => gap),
	);
});

test("Shift integrates the faster approach continuously across frame partitions", () => {
	const boosted: Movement = { ...FORWARD, boost: true };
	const speed = 3;
	const seconds = 0.04;
	const gap = 0.2;
	const full = 2 * stepSpeed(boosted, speed, seconds, () => gap) * seconds;
	const first =
		(2 * stepSpeed(boosted, speed, seconds / 2, () => gap) * seconds) / 2;
	const second =
		(2 * stepSpeed(boosted, speed, seconds / 2, () => gap - first) * seconds) /
		2;
	expect(Math.abs(full - travelled(speed, gap, seconds * 2))).toBeLessThan(
		1e-12,
	);
	expect(Math.abs(full - first - second)).toBeLessThan(1e-12);
});

test("one analytic query per active W step; misses keep speed and unresolved or invalid requests throw", () => {
	let calls = 0;
	function hit(): number {
		calls += 1;
		return 0.125;
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
