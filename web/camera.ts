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
const MIN_SPEED = 0.000_001;
const MAX_SPEED = 100;
const MAX_STEP = 0.05;
const APPROACH_RANGE = 1;
const POINTER_SCALE = 0.008;
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
	if (object.kind === "crystal") {
		return 1.75 * object.size;
	}
	return object.size * Math.sqrt(3 + Math.sqrt(3 * object.parameter));
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
	position: PureCamera = camera,
): PureCamera {
	finite(totalDx, "horizontal drag");
	finite(totalDy, "vertical drag");
	const turned = normalizeCamera({
		...camera,
		yaw: camera.yaw - totalDx * POINTER_SCALE,
		pitch: camera.pitch + totalDy * POINTER_SCALE,
	});
	const { origin } = cameraBasis(position);
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
function validStep(movement: Movement, speed: number, seconds: number): void {
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
}
function moveCamera(
	camera: PureCamera,
	movement: Movement,
	speed: number,
	seconds: number,
): PureCamera {
	validStep(movement, speed, seconds);
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
// Integrate v(gap) = minimum + gain * gap inside the band, at base speed outside.
// Crossing continues at the floor for the rest of this step: this is not collision.
function approachSpeed(
	speed: number,
	distance: number,
	seconds: number,
): number {
	if (speed === MIN_SPEED || distance >= APPROACH_RANGE + speed * seconds) {
		return speed;
	}
	const free = Math.min(
		seconds,
		Math.max(0, distance - APPROACH_RANGE) / speed,
	);
	const remaining = seconds - free;
	const near = Math.min(distance, APPROACH_RANGE);
	const gain = (speed - MIN_SPEED) / APPROACH_RANGE;
	const contact = Math.log1p((gain * near) / MIN_SPEED) / gain;
	let average: number;
	if (remaining <= contact) {
		const exponent = gain * remaining;
		let ratio = 1;
		if (exponent !== 0) {
			ratio = -Math.expm1(-exponent) / exponent;
		}
		average = (MIN_SPEED + gain * near) * ratio;
	} else {
		average = near / remaining + MIN_SPEED * (1 - contact / remaining);
	}
	return Math.min(
		speed,
		Math.max(
			MIN_SPEED,
			speed * (free / seconds) + average * (remaining / seconds),
		),
	);
}
function stepSpeed(
	movement: Movement,
	speed: number,
	seconds: number,
	centerHit: () => number | null,
): number {
	validStep(movement, speed, seconds);
	travelSpeed(speed, 0);
	if (
		seconds === 0 ||
		movement.onlyForward !== true ||
		movement.forward !== 1 ||
		movement.side !== 0
	) {
		return speed;
	}
	const distance = centerHit();
	if (distance === null) {
		return speed;
	}
	finite(distance, "center hit distance");
	if (distance < 0) {
		invalid("negative center hit distance");
	}
	return approachSpeed(speed, distance, Math.min(seconds, MAX_STEP));
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
	cameraBasis,
	lookCamera,
	moveCamera,
	stepSpeed,
	travelSpeed,
	STOPPED,
	MIN_SPEED,
	MAX_SPEED,
	type Movement,
};
