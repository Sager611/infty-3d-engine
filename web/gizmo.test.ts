// biome-ignore-all lint/style/noMagicNumbers: Analytic projection and drag fixtures.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Bun fixtures, not Qwik.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import {
	advanceDrag,
	arcPoint,
	beginDrag,
	type Drag,
	type Handle,
	layoutGizmo,
	makeProjection,
	nudge,
	type Pixel,
	type Projection,
	project,
} from "./gizmo.ts";
import { rotateVector } from "./rotation.ts";
import type { SceneObject } from "./scene.ts";

const required = <T>(value: T | undefined): T => {
	if (value === undefined) {
		throw new Error("Missing fixture value");
	}
	return value;
};
const object: SceneObject = {
	id: 1,
	kind: "finite",
	position: [0, 0, 0],
	size: 1,
	parameter: 0,
};
const projection: Projection = required(
	makeProjection(
		{ yaw: 0, pitch: 0, distance: 5, target: [0, 0, 0] },
		[1000, 800],
	),
);
const moveX: Handle = { mode: "move", axis: 0 };
const rotateZ: Handle = { mode: "rotate", axis: 2 };
const start = (handle: Handle, point: Pixel): Drag =>
	required(beginDrag(projection, object, handle, point));
const turnPoint = (angle: number): Pixel =>
	required(project(projection, arcPoint(object.position, 2, angle, 1)));
const apply = (
	drag: Drag,
	current: SceneObject,
	pixel: Pixel,
	shift: boolean,
): { drag: Drag; object: SceneObject } => {
	const next = advanceDrag(drag, current, pixel, shift);
	return { drag: next.drag, object: { ...current, ...next.patch } };
};

test("projection matches camera rays, display aspect and close-up target precision", () => {
	expect(project(projection, [1, 1, 0])).toEqual([644, 256]);
	const stretched = required(makeProjection(projection.camera, [1000, 800], 2));
	expect(project(stretched, [1, 1, 0])).toEqual([590, 256]);
	expect(project(projection, [0, 0, 6])).toBeUndefined();
	expect(project(projection, [Number.NaN, 0, 0])).toBeUndefined();
	expect(makeProjection(projection.camera, [0, 800])).toBeUndefined();
	expect(makeProjection(projection.camera, [1000, Number.NaN])).toBeUndefined();
	const tiny = required(
		makeProjection(
			{
				yaw: 0.28,
				pitch: 0.32,
				distance: 1e-12,
				target: [0, 1, 0],
			},
			[1000, 800],
		),
	);
	const center = required(project(tiny, [0, 1, 0]));
	expect(center[0]).toBeCloseTo(500, 9);
	expect(center[1]).toBeCloseTo(400, 9);
});

test("arrows start at the pivot; arcs stay between world axes and retain screen size", () => {
	const layout = required(layoutGizmo(projection, object.position));
	expect(layout.handles).toHaveLength(6);
	for (const item of layout.handles.filter(
		(entry) => entry.handle.mode === "move",
	)) {
		expect(item.points[0]).toEqual(layout.center);
	}
	expect(arcPoint([1, 2, 3], 2, 0, 2)).toEqual([3, 2, 3]);
	expect(arcPoint([1, 2, 3], 0, 0, 2)).toEqual([1, 4, 3]);
	const far = required(
		makeProjection({ ...projection.camera, distance: 50 }, [1000, 800]),
	);
	expect(
		required(layoutGizmo(far, object.position)).handles[0]?.points,
	).toEqual(layout.handles[0]?.points);
});

test("translation follows the selected world axis with no mutation", () => {
	const result = apply(start(moveX, [572, 400]), object, [586.4, 400], false);
	expect(result.object.position[0]).toBeCloseTo(0.1, 12);
	expect(result.object.position.slice(1)).toEqual([0, 0]);
	expect(object.position).toEqual([0, 0, 0]);
	const same = advanceDrag(result.drag, result.object, [586.4, 400], false);
	expect(same.patch).toBeUndefined();
});

test("Shift doubles actual translation increments and changes mid-drag without jumps", () => {
	let result = apply(start(moveX, [572, 400]), object, [586.4, 400], false);
	expect(result.object.position[0]).toBeCloseTo(0.1, 12);
	result = apply(result.drag, result.object, [600.8, 400], true);
	expect(result.object.position[0]).toBeCloseTo(0.3, 12);
	result = apply(result.drag, result.object, [600.8, 400], false);
	expect(result.object.position[0]).toBeCloseTo(0.3, 12);
	result = apply(result.drag, result.object, [615.2, 400], false);
	expect(result.object.position[0]).toBeCloseTo(0.4, 12);
});

test("rotation fixes the pivot and Shift doubles incremental angles, not the whole drag", () => {
	let result = apply(
		start(rotateZ, turnPoint(0.2)),
		object,
		turnPoint(0.3),
		false,
	);
	result = apply(result.drag, result.object, turnPoint(0.4), true);
	result = apply(result.drag, result.object, turnPoint(0.5), false);
	const direction = rotateVector(required(result.object.rotation), [1, 0, 0]);
	expect(direction[0]).toBeCloseTo(Math.cos(0.4), 12);
	expect(direction[1]).toBeCloseTo(Math.sin(0.4), 12);
	expect(direction[2]).toBeCloseTo(0, 12);
	expect(result.object.position).toEqual(object.position);
});

test("rotation crosses the angle branch cut by the short arc", () => {
	const result = apply(
		start(rotateZ, turnPoint(Math.PI - 0.01)),
		object,
		turnPoint(-Math.PI + 0.01),
		true,
	);
	const direction = rotateVector(required(result.object.rotation), [1, 0, 0]);
	expect(direction[0]).toBeCloseTo(Math.cos(0.04), 12);
	expect(direction[1]).toBeCloseTo(Math.sin(0.04), 12);
});

test("all world axes translate and rotate under an oblique camera", () => {
	const view = required(
		makeProjection(
			{
				yaw: 0.7,
				pitch: 0.4,
				distance: 5,
				target: [0, 0, 0],
			},
			[1000, 800],
		),
	);
	for (const axis of [0, 1, 2] as const) {
		const position: [number, number, number] = [0, 0, 0];
		position[axis] = 0.7;
		const initial: [number, number, number] = [0, 0, 0];
		initial[axis] = 0.6;
		const drag = required(
			beginDrag(
				view,
				object,
				{ mode: "move", axis },
				required(project(view, initial)),
			),
		);
		const next = advanceDrag(
			drag,
			object,
			required(project(view, position)),
			true,
		);
		expect(required(next.patch?.position)[axis]).toBeCloseTo(0.2, 12);
		const rotation = required(
			beginDrag(
				view,
				object,
				{ mode: "rotate", axis },
				required(project(view, arcPoint(object.position, axis, 0.3, 1))),
			),
		);
		const turn = advanceDrag(
			rotation,
			object,
			required(project(view, arcPoint(object.position, axis, 0.4, 1))),
			true,
		);
		expect(Math.hypot(...required(turn.patch?.rotation))).toBeCloseTo(1, 12);
	}
});

test("parallel, zero-radius, invalid and stale drags fail closed; bounds reverse immediately", () => {
	expect(
		beginDrag(projection, object, { mode: "move", axis: 2 }, [500, 400]),
	).toBeUndefined();
	expect(beginDrag(projection, object, rotateZ, [500, 400])).toBeUndefined();
	expect(beginDrag(projection, object, moveX, [Number.NaN, 0])).toBeUndefined();
	const drag = start(moveX, [572, 400]);
	expect(
		advanceDrag(drag, object, [Number.POSITIVE_INFINITY, 0], true).patch,
	).toBeUndefined();
	expect(
		advanceDrag(drag, { ...object, id: 2 }, [586.4, 400], false).patch,
	).toBeUndefined();
	const near = { ...object, position: [19.95, 0, 0] as const };
	const clamped = apply(drag, near, [586.4, 400], false);
	expect(clamped.object.position[0]).toBe(20);
	const reverse = apply(clamped.drag, clamped.object, [579.2, 400], false);
	expect(reverse.object.position[0]).toBeCloseTo(19.95, 12);
});

test("keyboard nudges are numeric fine translation and one-degree world turns, with Shift2x", () => {
	expect(nudge(object, moveX, 1).position).toEqual([0.001, 0, 0]);
	expect(nudge(object, moveX, 2).position).toEqual([0.002, 0, 0]);
	const turned = rotateVector(
		required(nudge(object, rotateZ, 2).rotation),
		[1, 0, 0],
	);
	expect(turned[1]).toBeCloseTo(Math.sin(Math.PI / 90), 12);
	expect(nudge(object, moveX, Number.NaN)).toEqual({});
});
