// biome-ignore lint/nursery/noUnresolvedImports: Bun test runtime.
import { expect, test } from "bun:test";
import {
	addInstance,
	createEditor,
	deleteInstances,
	duplicateInstances,
	undoEdit,
} from "./editing.ts";
import {
	DEFAULT_SCENE,
	MAX_OBJECTS,
	type Scene,
	updateObject,
} from "./scene.ts";
import { DEFAULT_SNAPSHOT, parseSnapshot, serializeSnapshot } from "./state.ts";

const DEFAULT_COUNT = 3;
const STALE_ID = 999;
const NEXT_ID = 4;
const BOUND = 20;
const MAX_ID = 0xff_ff_ff_ff;
const COPY_SIZE = 0.42;
const LEAF_SIZE = 0.07;
const GOURSAT_PARAMETER = 1.5;
const COPY_Z = 3;

test("add each kind, select fresh IDs, preserve camera and round-trip saved instances", () => {
	let editor = createEditor(DEFAULT_SCENE);
	const camera = structuredClone(DEFAULT_SNAPSHOT.camera);
	for (const kind of ["sphere", "goursat", "plane"] as const) {
		const previous = editor;
		editor = addInstance(editor, kind, camera);
		expect(editor.objects.length).toBe(previous.objects.length + 1);
		expect(editor.objects.at(-1)?.kind).toBe(kind);
		const spawned = editor.objects.at(-1);
		if (spawned === undefined) {
			throw new Error("spawn missing");
		}
		expect(editor.selection.ids).toEqual([spawned.id]);
		expect(editor.selection.active).toBe(editor.objects.at(-1)?.id ?? null);
		expect(editor.objects.at(-1)?.position.every(Number.isFinite)).toBe(true);
		expect(spawned.position[1]).toBeGreaterThanOrEqual(camera.target[1]);
	}
	expect(camera).toEqual(DEFAULT_SNAPSHOT.camera);
	const restored = parseSnapshot(
		serializeSnapshot({ ...DEFAULT_SNAPSHOT, objects: editor.objects }),
	);
	expect(restored.objects).toEqual(editor.objects);
	expect(new Set(editor.objects.map((o) => o.id)).size).toBe(
		editor.objects.length,
	);
	expect(DEFAULT_SCENE).toHaveLength(DEFAULT_COUNT);
});

const COPY_SCENE: Scene = [
	{
		id: MAX_ID,
		kind: "goursat",
		position: [1, 2, COPY_Z],
		size: COPY_SIZE,
		parameter: GOURSAT_PARAMETER,
		material: "brick",
		rotation: [0, 0, Math.SQRT1_2, Math.SQRT1_2],
	},
	{
		id: 1,
		kind: "finite",
		position: [BOUND, -BOUND, BOUND],
		size: LEAF_SIZE,
		parameter: 0,
		material: "leaf",
		rotation: [0, Math.SQRT1_2, 0, Math.SQRT1_2],
	},
	{ id: 2, kind: "sphere", position: [0, 0, 0], size: 1, parameter: 0 },
];
test("duplicate selected instances preserves pose/style, wraps IDs, selects copies and undoes once", () => {
	const objects = COPY_SCENE;
	const source = {
		...createEditor(objects),
		selection: { ids: [1, MAX_ID, 1, STALE_ID], active: 1 },
	};
	const copied = duplicateInstances(source);
	const firstId = DEFAULT_COUNT;
	expect(copied.objects.slice(0, objects.length)).toEqual([...objects]);
	expect(copied.objects.slice(objects.length)).toEqual(
		objects
			.slice(0, 2)
			.map((object, index) => ({ ...object, id: firstId + index })),
	);
	expect(copied.objects[objects.length]).not.toBe(objects[0]);
	expect(copied.selection).toEqual({
		ids: [firstId, NEXT_ID],
		active: NEXT_ID,
	});
	expect(copied.nextId).toBe(NEXT_ID + 1);
	expect(source.objects).toBe(objects);
	expect(source.nextId).toBe(MAX_ID + 1);
	const moved = updateObject(copied.objects, NEXT_ID, { position: [0, 0, 0] });
	expect(moved.find((object) => object.id === 1)?.position).toEqual([
		BOUND,
		-BOUND,
		BOUND,
	]);
	expect(
		copied.objects.find((object) => object.id === NEXT_ID)?.position,
	).toEqual([BOUND, -BOUND, BOUND]);
	expect(
		parseSnapshot(
			serializeSnapshot({ ...DEFAULT_SNAPSHOT, objects: copied.objects }),
		).objects,
	).toEqual(copied.objects);
	const undone = undoEdit(copied);
	expect(undone.objects).toBe(objects);
	expect(undone.selection).toEqual({ ids: [1, MAX_ID], active: 1 });
	expect(undone.undo).toBeNull();
	expect(undoEdit(undone)).toBe(undone);
});

test("duplicate handles empty/stale selection and keeps only the latest undo", () => {
	const editor = createEditor(DEFAULT_SCENE);
	expect(duplicateInstances(editor)).toEqual(editor);
	const deleted = deleteInstances({
		...editor,
		selection: { ids: [1], active: 1 },
	});
	expect(duplicateInstances(deleted)).toEqual(deleted);
	const stale = duplicateInstances({
		...deleted,
		selection: { ids: [1, STALE_ID], active: STALE_ID },
	});
	expect(stale.objects).toBe(deleted.objects);
	expect(stale.selection).toEqual({ ids: [], active: null });
	expect(stale.undo).toBe(deleted.undo);
	const first = duplicateInstances({
		...editor,
		selection: { ids: [1], active: 1 },
	});
	const second = duplicateInstances(first);
	expect(new Set(second.objects.map((object) => object.id)).size).toBe(
		second.objects.length,
	);
	expect(undoEdit(second).objects).toBe(first.objects);
	expect(undoEdit(second).selection).toEqual(first.selection);
	expect(undoEdit(undoEdit(second)).objects).toBe(first.objects);
});

test("delete/undo is bounded, handles stale multi-selection and last object without reusing live geometry", () => {
	let editor = createEditor(DEFAULT_SCENE);
	editor = {
		...editor,
		selection: { ids: [1, 2, STALE_ID, 2], active: STALE_ID },
	};
	const deleted = deleteInstances(editor);
	expect(deleted.objects.map((o) => o.id)).toEqual([DEFAULT_COUNT]);
	expect(deleted.selection).toEqual({ ids: [], active: null });
	const undone = undoEdit(deleted);
	expect(undone.objects).toEqual(DEFAULT_SCENE);
	expect(undone.selection).toEqual({ ids: [1, 2], active: 1 });
	expect(undone.undo).toBeNull();
	expect(undoEdit(undone)).toBe(undone);
	const empty = deleteInstances({
		...undone,
		selection: { ids: [1, 2, DEFAULT_COUNT], active: 1 },
	});
	expect(empty.objects).toEqual([]);
	expect(undoEdit(empty).objects).toEqual(DEFAULT_SCENE);
	const added = addInstance(empty, "sphere", DEFAULT_SNAPSHOT.camera);
	expect(added.undo).toBeNull();
	expect(added.objects).toHaveLength(1);
	expect(added.objects[0]?.id).toBe(NEXT_ID);
	expect(deleteInstances(createEditor([])).undo).toBeNull();
});

test("capacity is explicit, maximum IDs wrap safely and spawning near bounds stays valid", () => {
	const full: Scene = Array.from({ length: MAX_OBJECTS }, (_, i) => ({
		kind: "sphere",
		position: [0, 0, 0],
		size: 1,
		parameter: 0,
		id: i + 1,
	}));
	const fullEditor = createEditor(full);
	expect(() =>
		addInstance(fullEditor, "sphere", DEFAULT_SNAPSHOT.camera),
	).toThrow("capacity");
	expect(duplicateInstances(fullEditor)).toEqual(fullEditor);
	expect(() =>
		duplicateInstances({
			...fullEditor,
			selection: { ids: [1], active: 1 },
		}),
	).toThrow("capacity");
	expect(() =>
		duplicateInstances({
			...fullEditor,
			objects: full.slice(1),
			selection: { ids: [2, DEFAULT_COUNT], active: 2 },
		}),
	).toThrow("capacity");
	expect(fullEditor.objects).toHaveLength(MAX_OBJECTS);
	const exhausted = createEditor([
		{
			kind: "sphere",
			position: [0, 0, 0],
			size: 1,
			parameter: 0,
			id: MAX_ID,
		},
		...DEFAULT_SCENE,
	]);
	const wrapped = addInstance(exhausted, "sphere", DEFAULT_SNAPSHOT.camera);
	expect(wrapped.objects.map((object) => object.id)).toEqual([
		MAX_ID,
		...DEFAULT_SCENE.map((object) => object.id),
		NEXT_ID,
	]);
	const another = addInstance(wrapped, "goursat", DEFAULT_SNAPSHOT.camera);
	expect(another.objects.at(-1)?.id).toBe(NEXT_ID + 1);
	const edge = addInstance(createEditor([]), "goursat", {
		...DEFAULT_SNAPSHOT.camera,
		target: [BOUND, BOUND, BOUND],
	});
	expect(
		parseSnapshot(
			serializeSnapshot({ ...DEFAULT_SNAPSHOT, objects: edge.objects }),
		).objects,
	).toEqual(edge.objects);
});

const PORTRAIT_WIDTH = 390;
const PORTRAIT_HEIGHT = 844;
const PORTRAIT_ASPECT: number = PORTRAIT_WIDTH / PORTRAIT_HEIGHT;
const AXES = 3;
const FOCAL_LENGTH = 1.8;
const HALF = 0.5;
const ROUNDING = 1e-9;
test("finite spawns stay in the current view, including portrait and tilted cameras", () => {
	for (const aspect of [1, PORTRAIT_ASPECT]) {
		const view = { ...DEFAULT_SNAPSHOT.camera, pitch: 0.8, aspect };
		let editor = createEditor(DEFAULT_SCENE);
		for (const kind of ["sphere", "goursat"] as const) {
			editor = addInstance(editor, kind, view);
			const spawned = editor.objects.at(-1);
			if (spawned === undefined) {
				throw new Error("spawn missing");
			}
			let radius = spawned.size;
			if (kind === "goursat") {
				radius *= Math.sqrt(AXES + Math.sqrt(AXES * spawned.parameter));
			}
			const dx = spawned.position[0] - view.target[0];
			const dy = spawned.position[1] - view.target[1];
			const dz = spawned.position[2] - view.target[2];
			const horizontal = dx * Math.cos(view.yaw) - dz * Math.sin(view.yaw);
			const vertical =
				-dx * Math.sin(view.yaw) * Math.sin(view.pitch) +
				dy * Math.cos(view.pitch) -
				dz * Math.cos(view.yaw) * Math.sin(view.pitch);
			const depth =
				view.distance -
				dx * Math.sin(view.yaw) * Math.cos(view.pitch) -
				dy * Math.sin(view.pitch) -
				dz * Math.cos(view.yaw) * Math.cos(view.pitch);
			expect(Math.abs(horizontal) + radius).toBeLessThanOrEqual(
				((depth - radius) * HALF * aspect) / FOCAL_LENGTH + ROUNDING,
			);
			expect(Math.abs(vertical) + radius).toBeLessThanOrEqual(
				((depth - radius) * HALF) / FOCAL_LENGTH + ROUNDING,
			);
		}
	}
	expect(() =>
		addInstance(createEditor(DEFAULT_SCENE), "sphere", {
			...DEFAULT_SNAPSHOT.camera,
			aspect: Number.NaN,
		}),
	).toThrow("viewport");
	expect(() =>
		addInstance(createEditor(DEFAULT_SCENE), "goursat", {
			...DEFAULT_SNAPSHOT.camera,
			distance: 0.0001,
		}),
	).toThrow("Zoom out");
});
