// biome-ignore-all lint/style/noMagicNumbers: Camera contract constants are explicit.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
// biome-ignore-all lint/nursery/useMaxParams: Public camera operations use fixed contract parameters.
import type { SceneObject } from "./scene.ts";
import { normalizeCamera, type PureCamera } from "./state.ts";

type Vector = readonly [number, number, number];
type CameraBasis = Readonly<{ origin: Vector; forward: Vector; right: Vector }>;
type Movement = Readonly<{
	forward: number;
	side: number;
	onlyForward: boolean;
}>;
const STOPPED: Movement = Object.freeze({
	forward: 0,
	side: 0,
	onlyForward: false,
});
const MIN_SPEED = 0.01;
const MAX_SPEED = 100;
const MAX_STEP = 0.05;
const POINTER_SCALE = 0.008;
const MIN_DISTANCE = 0.0001;
const MAX_DISTANCE = 100;
const FOCAL_LENGTH = 1.8;
const FRAME_MARGIN = 1.1;

const invalid = (message: string): never => {
	throw new Error(`invalid camera: ${message}`);
};

const finite = (value: number, name: string): void => {
	if (!Number.isFinite(value)) {
		invalid(`nonfinite ${name}`);
	}
};

const clamp = (value: number, minimum: number, maximum: number): number =>
	Math.min(maximum, Math.max(minimum, value));

const isFiniteObject = (object: SceneObject): boolean =>
	Number.isFinite(object.id) &&
	Number.isFinite(object.position[0]) &&
	Number.isFinite(object.position[1]) &&
	Number.isFinite(object.position[2]) &&
	Number.isFinite(object.size) &&
	Number.isFinite(object.parameter);

const objectRadius = (object: SceneObject): number => {
	if (object.kind === "sphere") {
		return object.size;
	}
	return object.size * Math.sqrt(3 + Math.sqrt(3 * object.parameter));
};

const orbitCamera = (
	camera: PureCamera,
	totalDx: number,
	totalDy: number,
): PureCamera => {
	finite(totalDx, "horizontal drag");
	finite(totalDy, "vertical drag");
	return normalizeCamera({
		yaw: camera.yaw - totalDx * POINTER_SCALE,
		pitch: camera.pitch + totalDy * POINTER_SCALE,
		distance: camera.distance,
		target: camera.target,
	});
};

const zoomCamera = (
	camera: PureCamera,
	amount: number,
	hitDistance: number | null,
): PureCamera => {
	finite(amount, "zoom amount");
	if (hitDistance !== null) {
		finite(hitDistance, "hit distance");
		if (hitDistance < 0) {
			invalid("negative hit distance");
		}
	}
	const start = normalizeCamera(camera);
	const gap = Math.max(hitDistance ?? start.distance, MIN_DISTANCE);
	const scaledGap = clamp(gap * Math.exp(amount), MIN_DISTANCE, MAX_DISTANCE);
	return normalizeCamera({
		yaw: start.yaw,
		pitch: start.pitch,
		distance: start.distance - gap + scaledGap,
		target: start.target,
	});
};

const frameBounds = (
	objects: readonly SceneObject[],
): { radius: number; target: readonly [number, number, number] } => {
	let minimumX = Number.POSITIVE_INFINITY;
	let minimumY = Number.POSITIVE_INFINITY;
	let minimumZ = Number.POSITIVE_INFINITY;
	let maximumX = Number.NEGATIVE_INFINITY;
	let maximumY = Number.NEGATIVE_INFINITY;
	let maximumZ = Number.NEGATIVE_INFINITY;
	for (const object of objects) {
		const radius = objectRadius(object);
		minimumX = Math.min(minimumX, object.position[0] - radius);
		minimumY = Math.min(minimumY, object.position[1] - radius);
		minimumZ = Math.min(minimumZ, object.position[2] - radius);
		maximumX = Math.max(maximumX, object.position[0] + radius);
		maximumY = Math.max(maximumY, object.position[1] + radius);
		maximumZ = Math.max(maximumZ, object.position[2] + radius);
	}
	const target: readonly [number, number, number] = [
		(minimumX + maximumX) / 2,
		(minimumY + maximumY) / 2,
		(minimumZ + maximumZ) / 2,
	];
	let radius = 0;
	for (const object of objects) {
		radius = Math.max(
			radius,
			Math.hypot(
				object.position[0] - target[0],
				object.position[1] - target[1],
				object.position[2] - target[2],
			) + objectRadius(object),
		);
	}
	return { radius, target };
};

const frameCamera = (
	camera: PureCamera,
	objects: readonly SceneObject[],
	ids: readonly number[],
	aspect: number,
): PureCamera => {
	finite(aspect, "aspect");
	if (aspect <= 0 || aspect > 100) {
		invalid("aspect range");
	}
	for (const id of ids) {
		if (!objects.some((object) => object.id === id)) {
			invalid("unknown selected object");
		}
	}
	let selected: readonly SceneObject[];
	if (ids.length === 0) {
		selected = objects;
	} else {
		selected = objects.filter((object) => ids.includes(object.id));
	}
	const selectedObjects = selected.filter((object) => object !== undefined);
	const finiteSelected = selectedObjects.filter(isFiniteObject);
	const bounded = finiteSelected.filter(
		(object) =>
			object.kind !== "plane" &&
			Number.isFinite(objectRadius(object)) &&
			objectRadius(object) >= 0,
	);
	if (bounded.length === 0) {
		const plane = selectedObjects.find(
			(object) =>
				object.kind === "plane" &&
				object.position.every((value) => Number.isFinite(value)),
		);
		if (plane === undefined) {
			return camera;
		}
		return normalizeCamera({
			yaw: camera.yaw,
			pitch: camera.pitch,
			distance: camera.distance,
			target: plane.position,
		});
	}
	const { radius, target } = frameBounds(bounded);
	const verticalFov = 2 * Math.atan(0.5 / FOCAL_LENGTH);
	const horizontalFov = 2 * Math.atan((aspect * 0.5) / FOCAL_LENGTH);
	const fitFov = Math.min(horizontalFov, verticalFov);
	const distance = (radius / Math.tan(fitFov / 2)) * FRAME_MARGIN;
	return normalizeCamera({
		yaw: camera.yaw,
		pitch: camera.pitch,
		distance,
		target,
	});
};

function cameraBasis(camera: PureCamera): CameraBasis {
	const c = normalizeCamera(camera);
	const forward: Vector = [
		-Math.cos(c.pitch) * Math.sin(c.yaw),
		-Math.sin(c.pitch),
		-Math.cos(c.pitch) * Math.cos(c.yaw),
	];
	return {
		origin: [
			c.target[0] - c.distance * forward[0],
			c.target[1] - c.distance * forward[1],
			c.target[2] - c.distance * forward[2],
		],
		forward,
		right: [Math.cos(c.yaw), 0, -Math.sin(c.yaw)],
	};
}
function lookCamera(
	camera: PureCamera,
	totalDx: number,
	totalDy: number,
): PureCamera {
	const turned = orbitCamera(camera, totalDx, totalDy);
	const { origin } = cameraBasis(camera);
	const { forward } = cameraBasis(turned);
	return normalizeCamera({
		...turned,
		target: [
			origin[0] + turned.distance * forward[0],
			origin[1] + turned.distance * forward[1],
			origin[2] + turned.distance * forward[2],
		],
	});
}
function moveCamera(
	camera: PureCamera,
	movement: Movement,
	speed: number,
	seconds: number,
): PureCamera {
	finite(speed, "travel speed");
	finite(seconds, "elapsed time");
	finite(movement.forward, "forward movement");
	finite(movement.side, "side movement");
	if (
		speed <= 0 ||
		seconds < 0 ||
		Math.abs(movement.forward) > 1 ||
		Math.abs(movement.side) > 1
	) {
		invalid("travel step");
	}
	const basis = cameraBasis(camera);
	const length = Math.hypot(movement.forward, movement.side);
	if (length === 0 || seconds === 0) {
		return camera;
	}
	const distance = (speed * Math.min(seconds, MAX_STEP)) / length;
	return normalizeCamera({
		...camera,
		target: [
			camera.target[0] +
				distance *
					(movement.forward * basis.forward[0] +
						movement.side * basis.right[0]),
			camera.target[1] +
				distance *
					(movement.forward * basis.forward[1] +
						movement.side * basis.right[1]),
			camera.target[2] +
				distance *
					(movement.forward * basis.forward[2] +
						movement.side * basis.right[2]),
		],
	});
}
function travelSpeed(speed: number, amount: number): number {
	finite(speed, "travel speed");
	finite(amount, "speed adjustment");
	if (speed < MIN_SPEED || speed > MAX_SPEED) {
		invalid("travel speed");
	}
	if (amount === 0) {
		return speed;
	}
	const exponent = Math.log(speed) - amount;
	if (exponent <= Math.log(MIN_SPEED)) {
		return MIN_SPEED;
	}
	if (exponent >= Math.log(MAX_SPEED)) {
		return MAX_SPEED;
	}
	return Math.exp(exponent);
}
export {
	FOCAL_LENGTH,
	objectRadius,
	frameCamera,
	orbitCamera,
	zoomCamera,
	cameraBasis,
	lookCamera,
	moveCamera,
	travelSpeed,
	STOPPED,
	MIN_SPEED,
	MAX_SPEED,
	type Movement,
};
