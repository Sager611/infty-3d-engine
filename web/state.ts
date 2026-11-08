// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
// biome-ignore-all lint/correctness/useImportExtensions: Project modules use extensionless imports.
// biome-ignore-all lint/style/noMagicNumbers: Wire-format constants are contract values.
// biome-ignore-all lint/nursery/useExplicitType: Constants remain simple contract values.
// biome-ignore-all lint/nursery/noTernary: Codec branches stay compact.
// biome-ignore-all lint/complexity/useSimplifiedLogicExpression: Validation names both conditions.
import {
	DEFAULT_SCENE,
	packScene,
	type SceneObject,
	unpackScene,
} from "./scene";
import { parseView, type ViewMode } from "./view";

type PureCamera = Readonly<{
	yaw: number;
	pitch: number;
	distance: number;
	target: readonly [number, number, number];
}>;

type Snapshot = Readonly<{
	camera: PureCamera;
	mode: ViewMode;
	objects: readonly SceneObject[];
}>;

const MIN_PITCH = -1.45;
const MAX_PITCH = 1.45;
const MIN_DISTANCE = 0.0001;
const MAX_DISTANCE = 100;
const MIN_TARGET = -20;
const MAX_TARGET = 20;
const PI = Math.PI;

const cameraError = (message: string): never => {
	throw new Error(`invalid camera: ${message}`);
};

const validMode = (mode: number): mode is ViewMode =>
	mode === 0 || mode === 1 || mode === 2;

const normalizedYaw = (yaw: number): number => {
	if (yaw >= -PI && yaw <= PI) {
		return yaw;
	}
	const wrapped = ((((yaw + PI) % (2 * PI)) + 2 * PI) % (2 * PI)) - PI;
	return wrapped === -PI ? PI : wrapped;
};

const normalizeCamera = (camera: PureCamera): PureCamera => {
	const values = [
		camera.yaw,
		camera.pitch,
		camera.distance,
		camera.target[0],
		camera.target[1],
		camera.target[2],
	];
	if (values.some((value) => !Number.isFinite(value))) {
		cameraError("nonfinite");
	}
	if (camera.target.some((value) => value < MIN_TARGET || value > MAX_TARGET)) {
		cameraError("target");
	}
	return Object.freeze({
		yaw: normalizedYaw(camera.yaw),
		pitch: Math.min(MAX_PITCH, Math.max(MIN_PITCH, camera.pitch)),
		distance: Math.min(MAX_DISTANCE, Math.max(MIN_DISTANCE, camera.distance)),
		target: Object.freeze([
			camera.target[0],
			camera.target[1],
			camera.target[2],
		] as const),
	});
};

const cameraPacket = (
	camera: PureCamera,
): readonly [number, number, number, number, number, number] => {
	const normalized = normalizeCamera(camera);
	return Object.freeze([
		normalized.yaw,
		normalized.pitch,
		normalized.distance,
		normalized.target[0],
		normalized.target[1],
		normalized.target[2],
	]);
};

const strictCamera = (fields: readonly string[]): PureCamera => {
	if (fields.length !== 6 || fields.some((field) => field === "")) {
		cameraError("invalid length");
	}
	const values = fields.map(Number);
	if (values.some((value) => !Number.isFinite(value))) {
		cameraError("nonfinite");
	}
	const yaw = Number(fields[0] ?? "");
	const pitch = Number(fields[1] ?? "");
	const distance = Number(fields[2] ?? "");
	const x = Number(fields[3] ?? "");
	const y = Number(fields[4] ?? "");
	const z = Number(fields[5] ?? "");
	if (
		pitch < MIN_PITCH ||
		pitch > MAX_PITCH ||
		distance < MIN_DISTANCE ||
		distance > MAX_DISTANCE ||
		x < MIN_TARGET ||
		x > MAX_TARGET ||
		y < MIN_TARGET ||
		y > MAX_TARGET ||
		z < MIN_TARGET ||
		z > MAX_TARGET
	) {
		cameraError("bounds");
	}
	return normalizeCamera({ yaw, pitch, distance, target: [x, y, z] });
};

const parseSnapshot = (hash: string): Snapshot => {
	if (hash.startsWith("#v=")) {
		const view = parseView(hash);
		const sphere = DEFAULT_SCENE.find((object) => object.id === 1);
		if (sphere === undefined) {
			throw new Error("default scene has no sphere");
		}
		const target: readonly [number, number, number] = [0, view.radius, 0];
		return Object.freeze({
			camera: normalizeCamera({
				yaw: view.yaw,
				pitch: view.pitch,
				distance: view.distance,
				target,
			}),
			mode: view.mode,
			objects: Object.freeze([
				Object.freeze({
					...sphere,
					size: view.radius,
					position: Object.freeze(target),
				}),
				...DEFAULT_SCENE.filter((object) => object.kind === "plane"),
			]),
		});
	}
	if (!hash.startsWith("#s=")) {
		return DEFAULT_SNAPSHOT;
	}
	const parts = hash.slice(3).split(";");
	if (parts.length !== 4 || parts[0] !== "1") {
		throw new Error("invalid snapshot");
	}
	const camera = strictCamera((parts[1] ?? "").split(","));
	if (parts[2] === "") {
		throw new Error("invalid mode");
	}
	const modeValue = Number(parts[2] ?? "");
	if (!Number.isInteger(modeValue) || !validMode(modeValue)) {
		throw new Error("invalid mode");
	}
	const sceneText = parts[3] ?? "";
	const sceneValues =
		sceneText === ""
			? []
			: sceneText.split(",").map((field) => {
					if (field === "") {
						throw new Error("invalid scene");
					}
					return Number(field);
				});
	const objects = unpackScene(sceneValues);
	return Object.freeze({ camera, mode: modeValue, objects });
};

const serializeSnapshot = (snapshot: Snapshot): string => {
	if (!validMode(snapshot.mode)) {
		throw new Error("invalid mode");
	}
	const camera = cameraPacket(snapshot.camera);
	const scene = packScene(snapshot.objects).join(",");
	return `#s=1;${camera.join(",")};${snapshot.mode};${scene}`;
};

const DEFAULT_SNAPSHOT: Snapshot = Object.freeze({
	camera: Object.freeze({
		yaw: 0.2,
		pitch: 0.18,
		distance: 6.5,
		target: Object.freeze([0, 1.1, 0] as const),
	}),
	mode: 0,
	objects: DEFAULT_SCENE,
});

export {
	DEFAULT_SNAPSHOT,
	cameraPacket,
	normalizeCamera,
	parseSnapshot,
	serializeSnapshot,
	type PureCamera,
	type Snapshot,
};
