import { FOCAL_LENGTH, objectRadius } from "./camera.ts";
import { EMPTY_SELECTION, type Selection } from "./query.ts";
import {
	MAX_OBJECTS,
	MAX_POSITION,
	MIN_POSITION,
	type Position,
	packScene,
	type Scene,
	type SceneKind,
	type SceneObject,
} from "./scene.ts";
import type { PureCamera } from "./state.ts";

type SpawnView = PureCamera & Readonly<{ aspect?: number }>;
type Deleted = Readonly<{ objects: Scene; selection: Selection }>;
type Editor = Readonly<{
	objects: Scene;
	selection: Selection;
	nextId: number;
	undo: Deleted | null;
}>;
const MAX_ID = 0xff_ff_ff_ff;
const DEFAULT_PARAMETER = 1.5;
const MIN_SPAWN_SIZE = 0.01;
const MAX_SPAWN_SIZE = 0.6;
const VIEW_SCALE = 0.05;
const SPAWN_ATTEMPTS = 24;
const GOLDEN_ANGLE = 2.399_963_229_728_653;
const SPACING = 3;

function cleanSelection(objects: Scene, selection: Selection): Selection {
	const ids = new Set(objects.map((o) => o.id));
	const selected = [...new Set(selection.ids)].filter((id) => ids.has(id));
	let active = selected[0] ?? null;
	if (selection.active !== null && selected.includes(selection.active)) {
		active = selection.active;
	}
	return { ids: selected, active };
}
function createEditor(objects: Scene): Editor {
	packScene(objects);
	let nextId = 1;
	for (const object of objects) {
		nextId = Math.max(nextId, object.id + 1);
	}
	return { objects, selection: EMPTY_SELECTION, nextId, undo: null };
}
function clampPosition(value: number): number {
	return Math.max(MIN_POSITION, Math.min(MAX_POSITION, value));
}
function spawnLimits(
	camera: SpawnView,
	radius: number,
): readonly [horizontal: number, vertical: number] {
	const aspect = camera.aspect ?? 1;
	if (!Number.isFinite(aspect) || aspect <= 0) {
		throw new Error("Invalid viewport aspect");
	}
	const vertical = (camera.distance - radius) / (2 * FOCAL_LENGTH) - radius;
	const horizontal =
		((camera.distance - radius) * aspect) / (2 * FOCAL_LENGTH) - radius;
	if (horizontal < 0 || vertical < 0) {
		throw new Error("Zoom out before adding geometry");
	}
	return [horizontal, vertical];
}
function spawnPosition(
	editor: Editor,
	camera: SpawnView,
	shape: SceneObject,
): Position {
	let radius = 0;
	if (shape.kind !== "plane") {
		radius = objectRadius(shape);
	}
	const [horizontalLimit, verticalLimit] = spawnLimits(camera, radius);
	const others: readonly Readonly<{ position: Position; radius: number }>[] =
		editor.objects
			.filter((object) => object.kind !== "plane")
			.map((object) => ({
				position: object.position,
				radius: objectRadius(object),
			}));
	const cosYaw = Math.cos(camera.yaw);
	const sinYaw = Math.sin(camera.yaw);
	const cosPitch = Math.cos(camera.pitch);
	const sinPitch = Math.sin(camera.pitch);
	let best: Position = camera.target;
	let clearance = Number.NEGATIVE_INFINITY;
	for (let attempt = 0; attempt < SPAWN_ATTEMPTS; attempt += 1) {
		const angle = attempt * GOLDEN_ANGLE;
		const offset = Math.sqrt(attempt) * shape.size * SPACING;
		const horizontal = Math.max(
			-horizontalLimit,
			Math.min(horizontalLimit, Math.cos(angle) * offset),
		);
		const vertical = Math.min(
			verticalLimit,
			Math.abs(Math.sin(angle)) * offset,
		);
		const candidate: Position = [
			clampPosition(
				camera.target[0] + cosYaw * horizontal - sinYaw * sinPitch * vertical,
			),
			clampPosition(camera.target[1] + cosPitch * vertical),
			clampPosition(
				camera.target[2] - sinYaw * horizontal - cosYaw * sinPitch * vertical,
			),
		];
		let score = Number.POSITIVE_INFINITY;
		for (const other of others) {
			const distance = Math.hypot(
				...candidate.map((x, i) => x - (other.position[i] ?? x)),
			);
			score = Math.min(score, distance - radius - other.radius);
		}
		if (score > clearance) {
			best = candidate;
			clearance = score;
		}
		if (score >= 0 || shape.kind === "plane") {
			break;
		}
	}
	return best;
}
function addInstance(
	editor: Editor,
	kind: SceneKind,
	camera: SpawnView,
): Editor {
	if (editor.objects.length >= MAX_OBJECTS) {
		throw new Error("Scene capacity reached");
	}
	const ids = new Set(editor.objects.map((existing) => existing.id));
	let id = ((editor.nextId - 1) % MAX_ID) + 1;
	while (ids.has(id)) {
		id = (id % MAX_ID) + 1;
	}
	const size = Math.min(
		MAX_SPAWN_SIZE,
		Math.max(
			MIN_SPAWN_SIZE,
			camera.distance * VIEW_SCALE * Math.min(1, camera.aspect ?? 1),
		),
	);
	let parameter = 0;
	if (kind === "goursat") {
		parameter = DEFAULT_PARAMETER;
	}
	const object: SceneObject = {
		id,
		kind,
		position: camera.target,
		size,
		parameter,
	};
	return {
		objects: [
			...editor.objects,
			{ ...object, position: spawnPosition(editor, camera, object) },
		],
		selection: { ids: [object.id], active: object.id },
		nextId: object.id + 1,
		undo: null,
	};
}
function deleteInstances(editor: Editor): Editor {
	const selection = cleanSelection(editor.objects, editor.selection);
	if (selection.ids.length === 0) {
		return { ...editor, selection };
	}
	const ids = new Set(selection.ids);
	return {
		...editor,
		objects: editor.objects.filter((o) => !ids.has(o.id)),
		selection: EMPTY_SELECTION,
		undo: { objects: editor.objects, selection },
	};
}
function undoDeletion(editor: Editor): Editor {
	if (editor.undo === null) {
		return editor;
	}
	return {
		...editor,
		objects: editor.undo.objects,
		selection: editor.undo.selection,
		undo: null,
	};
}
export {
	createEditor,
	addInstance,
	deleteInstances,
	undoDeletion,
	cleanSelection,
	type Editor,
};
