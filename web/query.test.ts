// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Tests are not Qwik components.
// biome-ignore-all lint/style/noMagicNumbers: Protocol fixtures use exact contract values.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import {
	activateSelection,
	canvasPixel,
	decodeQueries,
	EMPTY_SELECTION,
	inspectResult,
	type QueryResult,
	selectionFromHits,
} from "./query.ts";

test("canvas picks discrete pixels and clamps the last pixel", () => {
	const rect = { left: 10, top: 20, width: 200, height: 100 };
	expect(canvasPixel([100, 50], rect, [25.4, 27.6])).toEqual([7, 3]);
	expect(canvasPixel([100, 50], rect, [210, 120])).toEqual([99, 49]);
	expect(canvasPixel([100, 50], rect, [-10, -10])).toEqual([0, 0]);
	expect(() =>
		canvasPixel([100, 50], { ...rect, width: 0 }, [10, 20]),
	).toThrow();
	expect(() => canvasPixel([100, 50], rect, [Number.NaN, 20])).toThrow();
});

test("inspection includes UV, normals and numerical error without exact quartic claims", () => {
	const [result] = decodeQueries(hit(1, 2));
	if (!result) {
		throw new Error("missing fixture");
	}
	const text = inspectResult({ ...result, kind: "goursat", uncertain: true });
	expect(text).toContain("uncertain hit");
	expect(text).toContain("UV");
	expect(text).toContain("normal");
	expect(text).toContain("error");
});

test("displayed gap limits round outward", () => {
	const [result] = decodeQueries(hit(1, 2));
	if (!result) {
		throw new Error("missing fixture");
	}
	expect(
		inspectResult({
			...result,
			hit: false,
			bounds: true,
			gapLower: 0.1239,
			gapUpper: 0.2341,
		}),
	).toContain("[0.123, 0.235]");
});

const packet = (records: readonly (readonly number[])[]): number[] =>
	records.flat();

const hit = (id: number, t: number): number[] => [
	id,
	1,
	1,
	t,
	0,
	0,
	1,
	2,
	3,
	0,
	1,
	0,
	0.25,
	0.75,
	4,
	5,
	6,
	7,
	8,
	9,
	0,
	0,
	0,
	0,
];

const miss = (id: number): number[] => [
	id,
	2,
	0,
	0,
	2,
	3,
	10,
	11,
	12,
	0,
	1,
	0,
	0,
	0,
	13,
	14,
	15,
	16,
	17,
	18,
	0.25,
	1,
	1,
	0,
];

describe("query decoding", () => {
	test("decodes a complete packet and keeps exact and bound gaps", () => {
		const results = decodeQueries(packet([hit(2, 3.5), miss(1)]));
		expect(results).toEqual([
			{
				id: 2,
				kind: "sphere",
				hit: true,
				t: 3.5,
				gapLower: 0,
				gapUpper: 0,
				point: [1, 2, 3],
				normal: [0, 1, 0],
				uv: [0.25, 0.75],
				rayPoint: [4, 5, 6],
				local: [7, 8, 9],
				error: 0,
				bounds: false,
				uncertain: false,
			},
			{
				id: 1,
				kind: "goursat",
				hit: false,
				t: 0,
				gapLower: 2,
				gapUpper: 3,
				point: [10, 11, 12],
				normal: [0, 1, 0],
				uv: [0, 0],
				rayPoint: [13, 14, 15],
				local: [16, 17, 18],
				error: 0.25,
				bounds: true,
				uncertain: true,
			},
		]);
		expect(Object.isFrozen(results)).toBe(true);
		expect(Object.isFrozen(results[0])).toBe(true);
		expect(Object.isFrozen(results[0]?.point)).toBe(true);
	});

	test("decodes empty and preserves first/all packet order", () => {
		expect(decodeQueries([])).toEqual([]);
		const results = decodeQueries(packet([hit(7, 2), hit(3, 1)]));
		expect(results.map((result) => result.id)).toEqual([7, 3]);
	});
});

describe("query validation", () => {
	test("rejects malformed length and metadata", () => {
		expect(() => decodeQueries(new Array(23).fill(0))).toThrow("length");
		const invalidId = hit(1, 1);
		invalidId[0] = 1.5;
		expect(() => decodeQueries(invalidId)).toThrow("id");
		const invalidKind = hit(1, 1);
		invalidKind[1] = 9;
		expect(() => decodeQueries(invalidKind)).toThrow("kind");
		const invalidHit = hit(1, 1);
		invalidHit[2] = 2;
		expect(() => decodeQueries(invalidHit)).toThrow("flag");
		const invalidGapKind = hit(1, 1);
		invalidGapKind[21] = 2;
		expect(() => decodeQueries(invalidGapKind)).toThrow("bounds");
		const invalidStatus = hit(1, 1);
		invalidStatus[22] = 2;
		expect(() => decodeQueries(invalidStatus)).toThrow("status");
		const invalidPadding = hit(1, 1);
		invalidPadding[23] = 2;
		expect(() => decodeQueries(invalidPadding)).toThrow("padding");
		const nonfinite = hit(1, 1);
		nonfinite[6] = Number.NaN;
		expect(() => decodeQueries(nonfinite)).toThrow("finite");
		const negativeDistance = miss(1);
		negativeDistance[4] = -1;
		expect(() => decodeQueries(negativeDistance)).toThrow("distance");
		const reversedBounds = miss(1);
		reversedBounds[5] = 1;
		expect(() => decodeQueries(reversedBounds)).toThrow("bounds");
		const uncomputedError = miss(1);
		uncomputedError[20] = Number.MAX_VALUE;
		expect(() => decodeQueries(uncomputedError)).toThrow("uncomputed");
	});

	test("rejects uncomputed status and hit gaps", () => {
		const uncomputed = miss(1);
		uncomputed[22] = 2;
		expect(() => decodeQueries(uncomputed)).toThrow("status");
		const hitGap = hit(1, 1);
		hitGap[4] = 0.1;
		hitGap[5] = 0.1;
		expect(() => decodeQueries(hitGap)).toThrow("hit");
	});
});

describe("selection", () => {
	test("selection is unique, immutable, and activatable only by selected IDs", () => {
		const first: QueryResult = {
			id: 4,
			kind: "sphere",
			hit: true,
			t: 1,
			gapLower: 0,
			gapUpper: 0,
			point: [1, 2, 3],
			normal: [0, 1, 0],
			uv: [0.25, 0.75],
			rayPoint: [4, 5, 6],
			local: [7, 8, 9],
			error: 0,
			bounds: false,
			uncertain: false,
		};
		const duplicate: QueryResult = { ...first, t: 2 };
		const selection = selectionFromHits([
			first,
			duplicate,
			{ ...first, id: 9 },
			{ ...first, id: 7, hit: false },
		]);
		expect(selection).toEqual({ ids: [4, 9], active: 4 });
		expect(Object.isFrozen(selection)).toBe(true);
		expect(Object.isFrozen(selection.ids)).toBe(true);
		expect(activateSelection(selection, 4)).toEqual({ ids: [4, 9], active: 4 });
		expect(() => activateSelection(selection, 99)).toThrow("selected");
		expect(EMPTY_SELECTION).toEqual({ ids: [], active: null });
		expect(Object.isFrozen(EMPTY_SELECTION)).toBe(true);
	});
});
