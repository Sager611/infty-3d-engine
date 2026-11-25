// biome-ignore-all lint/style/noMagicNumbers: Analytic camera/axis contracts and fixed handle dimensions.
// biome-ignore-all lint/nursery/useMaxParams: Pure operations receive explicit immutable drag inputs.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Pure math, not Qwik.
import { cameraBasis, FOCAL_LENGTH } from "./camera.ts";
import {
	type Axis,
	IDENTITY_ROTATION,
	turnRotation,
	type Vector,
} from "./rotation.ts";
import {
	MAX_POSITION,
	MIN_POSITION,
	type SceneObject,
	type ScenePatch,
} from "./scene.ts";
import { normalizeCamera, type PureCamera } from "./state.ts";

type Pixel = readonly [number, number];
type Handle = Readonly<{ mode: "move" | "rotate"; axis: Axis }>;
type Projection = Readonly<{
	camera: PureCamera;
	forward: Vector;
	right: Vector;
	up: Vector;
	width: number;
	height: number;
	sx: number;
	sy: number;
}>;
type Drag = Readonly<{
	projection: Projection;
	original: SceneObject;
	handle: Handle;
	sample: number;
}>;
type HandleGeometry = Readonly<{ handle: Handle; points: readonly Pixel[] }>;
type GizmoLayout = Readonly<{
	center: Pixel;
	handles: readonly HandleGeometry[];
}>;
const AXIS_IDS = [0, 1, 2] as const;
const PLANES = [
	[1, 2],
	[2, 0],
	[0, 1],
] as const;
const HANDLE_PIXELS = 88;
const ARC_STEPS = 16;
const ZERO: Vector = [0, 0, 0];
const dot = (a: Vector, b: Vector): number =>
	a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

const makeProjection = (
	input: PureCamera,
	size: Pixel,
	aspect: number = size[0] / size[1],
): Projection | undefined => {
	if (
		![...size, aspect].every(Number.isFinite) ||
		size[0] <= 0 ||
		size[1] <= 0 ||
		aspect <= 0
	) {
		return;
	}
	try {
		const camera = normalizeCamera(input);
		const { forward, right } = cameraBasis(camera);
		return {
			camera,
			forward,
			right,
			up: [
				right[1] * forward[2] - right[2] * forward[1],
				right[2] * forward[0] - right[0] * forward[2],
				right[0] * forward[1] - right[1] * forward[0],
			],
			width: size[0],
			height: size[1],
			sx: (FOCAL_LENGTH * size[0]) / (2 * aspect),
			sy: (FOCAL_LENGTH * size[1]) / 2,
		};
	} catch {
		return;
	}
};

// Subtract the target before adding eye offset: don't lose close-up precision.
const relative = (view: Projection, pivot: Vector): Vector => [
	view.camera.target[0] - pivot[0] - view.forward[0] * view.camera.distance,
	view.camera.target[1] - pivot[1] - view.forward[1] * view.camera.distance,
	view.camera.target[2] - pivot[2] - view.forward[2] * view.camera.distance,
];
const projectOffset = (
	view: Projection,
	pivot: Vector,
	offset: Vector,
): Pixel | undefined => {
	if (![...pivot, ...offset].every(Number.isFinite)) {
		return;
	}
	const origin = relative(view, pivot);
	const vector: Vector = [
		offset[0] - origin[0],
		offset[1] - origin[1],
		offset[2] - origin[2],
	];
	const depth = dot(vector, view.forward);
	if (!(depth > 0)) {
		return;
	}
	const pixel: Pixel = [
		view.width / 2 + (view.sx * dot(vector, view.right)) / depth,
		view.height / 2 - (view.sy * dot(vector, view.up)) / depth,
	];
	if (pixel.every(Number.isFinite)) {
		return pixel;
	}
	return;
};
const project = (view: Projection, point: Vector): Pixel | undefined =>
	projectOffset(view, point, ZERO);
const direction = (view: Projection, pixel: Pixel): Vector | undefined => {
	if (!pixel.every(Number.isFinite)) {
		return;
	}
	const x = (pixel[0] - view.width / 2) / view.sx;
	const y = (view.height / 2 - pixel[1]) / view.sy;
	const vector: Vector = [
		view.forward[0] + x * view.right[0] + y * view.up[0],
		view.forward[1] + x * view.right[1] + y * view.up[1],
		view.forward[2] + x * view.right[2] + y * view.up[2],
	];
	const length = Math.hypot(...vector);
	if (!(Number.isFinite(length) && length > 0)) {
		return;
	}
	return [vector[0] / length, vector[1] / length, vector[2] / length];
};
const arcOffset = (axis: Axis, angle: number, radius: number): Vector => {
	const offset: [number, number, number] = [0, 0, 0];
	const [u, v] = PLANES[axis];
	offset[u] = Math.cos(angle) * radius;
	offset[v] = Math.sin(angle) * radius;
	return offset;
};
const arcPoint = (
	pivot: Vector,
	axis: Axis,
	angle: number,
	radius: number,
): Vector => {
	const offset = arcOffset(axis, angle, radius);
	return [pivot[0] + offset[0], pivot[1] + offset[1], pivot[2] + offset[2]];
};
const layoutGizmo = (
	view: Projection,
	pivot: Vector,
): GizmoLayout | undefined => {
	const center = project(view, pivot);
	if (center === undefined) {
		return;
	}
	const length =
		(-dot(relative(view, pivot), view.forward) * HANDLE_PIXELS) / view.sy;
	const handles: HandleGeometry[] = [];
	for (const axis of AXIS_IDS) {
		const offset: [number, number, number] = [0, 0, 0];
		offset[axis] = length;
		const tip = projectOffset(view, pivot, offset);
		if (tip === undefined) {
			return;
		}
		handles.push({ handle: { mode: "move", axis }, points: [center, tip] });
		const points: Pixel[] = [];
		for (let step = 0; step <= ARC_STEPS; step += 1) {
			const angle = 0.13 + ((Math.PI / 2 - 0.26) * step) / ARC_STEPS;
			const point = projectOffset(
				view,
				pivot,
				arcOffset(axis, angle, length * 0.72),
			);
			if (point === undefined) {
				return;
			}
			points.push(point);
		}
		handles.push({ handle: { mode: "rotate", axis }, points });
	}
	return { center, handles };
};

// Axis: closest approach of two lines. Rotation: one ray/axis-plane intersection.
const sample = (
	view: Projection,
	pivot: Vector,
	handle: Handle,
	pixel: Pixel,
): number | undefined => {
	const ray = direction(view, pixel);
	if (ray === undefined) {
		return;
	}
	const origin = relative(view, pivot);
	const cosine = ray[handle.axis];
	if (handle.mode === "move") {
		const denominator = 1 - cosine * cosine;
		if (denominator < 1e-8) {
			return;
		}
		const alongRay = dot(ray, origin);
		const value = (origin[handle.axis] - cosine * alongRay) / denominator;
		if (Number.isFinite(value) && cosine * value - alongRay > 0) {
			return value;
		}
		return;
	}
	if (Math.abs(cosine) < 1e-8) {
		return;
	}
	const time = -origin[handle.axis] / cosine;
	if (!(time > 0 && Number.isFinite(time))) {
		return;
	}
	const [u, v] = PLANES[handle.axis];
	const x = origin[u] + ray[u] * time;
	const y = origin[v] + ray[v] * time;
	if (!(Math.hypot(x, y) > 0)) {
		return;
	}
	return Math.atan2(y, x);
};
const beginDrag = (
	view: Projection,
	object: SceneObject,
	handle: Handle,
	pixel: Pixel,
): Drag | undefined => {
	const value = sample(view, object.position, handle, pixel);
	if (value === undefined) {
		return;
	}
	return { projection: view, original: object, handle, sample: value };
};
const poseDelta = (
	object: SceneObject,
	handle: Handle,
	amount: number,
): ScenePatch | undefined => {
	if (!Number.isFinite(amount) || amount === 0) {
		return;
	}
	if (handle.mode === "rotate") {
		return {
			rotation: turnRotation(
				object.rotation ?? IDENTITY_ROTATION,
				handle.axis,
				amount,
			),
		};
	}
	const position: [number, number, number] = [...object.position];
	position[handle.axis] = Math.max(
		MIN_POSITION,
		Math.min(MAX_POSITION, position[handle.axis] + amount),
	);
	if (position[handle.axis] === object.position[handle.axis]) {
		return;
	}
	return { position };
};
const advanceDrag = (
	drag: Drag,
	object: SceneObject,
	pixel: Pixel,
	boost: boolean,
): Readonly<{ drag: Drag; patch?: ScenePatch }> => {
	if (object.id !== drag.original.id) {
		return { drag };
	}
	const value = sample(
		drag.projection,
		drag.original.position,
		drag.handle,
		pixel,
	);
	if (value === undefined) {
		return { drag };
	}
	let delta = value - drag.sample;
	if (drag.handle.mode === "rotate") {
		delta = Math.atan2(Math.sin(delta), Math.cos(delta));
	}
	const next = { ...drag, sample: value };
	const patch = poseDelta(object, drag.handle, delta * (1 + Number(boost)));
	if (patch === undefined) {
		return { drag: next };
	}
	return { drag: next, patch };
};
const NUDGE: Readonly<Record<Handle["mode"], number>> = {
	move: 0.001,
	rotate: Math.PI / 180,
};
const nudge = (
	object: SceneObject,
	handle: Handle,
	steps: number,
): ScenePatch => poseDelta(object, handle, steps * NUDGE[handle.mode]) ?? {};

export {
	AXIS_IDS,
	advanceDrag,
	arcPoint,
	beginDrag,
	layoutGizmo,
	makeProjection,
	nudge,
	project,
	type Drag,
	type Handle,
	type HandleGeometry,
	type Pixel,
	type Projection,
};
