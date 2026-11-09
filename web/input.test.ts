// biome-ignore-all lint/style/noMagicNumbers: Input contract values are asserted directly.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import { isClick, wheelAmount } from "./input.ts";

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

test("wheel units normalize pixels, lines and pages without accepting invalid input", () => {
	expect(wheelAmount(1, 0, 100)).toBe(0.001);
	expect(wheelAmount(1, 1, 100)).toBe(0.016);
	expect(wheelAmount(1, 2, 100)).toBe(0.1);
	expect(() => wheelAmount(Number.NaN, 0, 100)).toThrow();
	expect(() => wheelAmount(1, 3, 100)).toThrow();
	expect(() => wheelAmount(1, 2, 0)).toThrow();
});
