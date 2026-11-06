// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual bun:test module.
import { describe, expect, test } from "bun:test";
import {
	DEFAULT_VIEW,
	MAX_DISTANCE,
	MAX_PITCH,
	MAX_RADIUS,
	MAX_YAW,
	MIN_DISTANCE_GAP,
	MIN_RADIUS,
	MIN_YAW,
	parseView,
	serializeView,
	type View,
} from "./view.ts";

const TEST_RADIUS: number = MAX_RADIUS;

describe("view URL state defaults", () => {
	test("uses defaults for an empty or missing hash", () => {
		expect(parseView("")).toEqual(DEFAULT_VIEW);
		expect(parseView("#")).toEqual(DEFAULT_VIEW);
		expect(parseView("#v=")).toEqual(DEFAULT_VIEW);
	});

	test("falls back for malformed and nonfinite fields", () => {
		expect(parseView("#v=nope,Infinity,5,1,0")).toEqual({
			...DEFAULT_VIEW,
			distance: 5,
			radius: 1,
		});
		expect(parseView("#v=0,0,NaN,,0")).toEqual({
			yaw: 0,
			pitch: 0,
			distance: 5,
			radius: 1,
			mode: 0,
		});
		expect(parseView("#v=0,0,5,1")).toEqual({
			yaw: 0,
			pitch: 0,
			distance: 5,
			radius: 1,
			mode: 0,
		});
	});
});

describe("view URL state bounds", () => {
	test("clamps every bounded field", () => {
		expect(parseView("#v=-999,999,20,0.25,1")).toEqual({
			yaw: MIN_YAW,
			pitch: MAX_PITCH,
			distance: MAX_DISTANCE,
			radius: MIN_RADIUS,
			mode: 1,
		});
		expect(parseView("#v=999,-999,0,999,0")).toEqual({
			yaw: MAX_YAW,
			pitch: -MAX_PITCH,
			distance: MAX_RADIUS + MIN_DISTANCE_GAP,
			radius: MAX_RADIUS,
			mode: 0,
		});
	});

	test("clamps radius before enforcing the distance gap", () => {
		expect(parseView("#v=0,0,1,1.5,0")).toEqual({
			yaw: 0,
			pitch: 0,
			distance: TEST_RADIUS + MIN_DISTANCE_GAP,
			radius: TEST_RADIUS,
			mode: 0,
		});
	});
});

test("round-trips mode 2 and deep zoom", () => {
	const view: View = {
		yaw: -12.5,
		pitch: 1.25,
		distance: 0.7501,
		radius: 0.75,
		mode: 2,
	};
	expect(parseView(serializeView(view))).toEqual(view);
});

test("uses the default mode for unsupported modes", () => {
	expect(parseView("#v=0.1,0.2,5,1,3").mode).toBe(DEFAULT_VIEW.mode);
	expect(parseView("#v=0.1,0.2,5,1,-1").mode).toBe(DEFAULT_VIEW.mode);
});
