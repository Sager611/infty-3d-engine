// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
// biome-ignore-all lint/style/noMagicNumbers: Packet indices and locked defaults are contract values.
import {
	isMaterial,
	MATERIALS,
	type MaterialKind,
	materialKind,
} from "./material.ts";

import {
	IDENTITY_ROTATION,
	normalizeRotation,
	type Rotation,
	type Vector,
	validRotation,
} from "./rotation.ts";

type Position = Vector;
type SceneKind = "sphere" | "goursat" | "plane" | "crystal" | "finite";

type ReadonlySceneObject = Readonly<{
	id: number;
	kind: SceneKind;
	position: Position;
	size: number;
	parameter: number;
	material?: MaterialKind;
	rotation?: Rotation;
}>;

type SceneObject = ReadonlySceneObject;
type Scene = readonly ReadonlySceneObject[];
type ScenePatch = Readonly<{
	position?: Position;
	size?: number;
	parameter?: number;
	material?: MaterialKind;
	rotation?: Rotation;
}>;

type NumericRange = Readonly<{ min: number; max: number }>;
type ShapeDescriptor<K extends SceneKind = SceneKind> = Readonly<{
	kind: K;
	position: readonly ["x", "y", "z"];
	size: NumericRange;
	parameter: NumericRange;
}>;

type ShapeDescriptorMap = Readonly<{
	[K in SceneKind]: ShapeDescriptor<K>;
}>;

const MAX_OBJECTS = 65_536;
const MIN_POSITION = -20;
const MAX_POSITION = 20;
const MIN_SIZE = 0.01;
const MAX_SIZE = 4;
const MIN_GOURSAT_PARAMETER = 1.05;
const MAX_GOURSAT_PARAMETER = 1.95;
const PACKET_STRIDE = 8;
const POSE_PACKET_TAG = -1;
const POSE_STRIDE = 12;

const DESCRIPTORS: ShapeDescriptorMap = {
	sphere: {
		kind: "sphere",
		position: ["x", "y", "z"],
		size: { min: MIN_SIZE, max: MAX_SIZE },
		parameter: { min: 0, max: 0 },
	},
	goursat: {
		kind: "goursat",
		position: ["x", "y", "z"],
		size: { min: MIN_SIZE, max: MAX_SIZE },
		parameter: {
			min: MIN_GOURSAT_PARAMETER,
			max: MAX_GOURSAT_PARAMETER,
		},
	},
	plane: {
		kind: "plane",
		position: ["x", "y", "z"],
		size: { min: MIN_SIZE, max: MAX_SIZE },
		parameter: { min: 0, max: 0 },
	},
	crystal: {
		kind: "crystal",
		position: ["x", "y", "z"],
		size: { min: MIN_SIZE, max: MAX_SIZE },
		parameter: { min: 0, max: 0 },
	},
	finite: {
		kind: "finite",
		position: ["x", "y", "z"],
		size: { min: MIN_SIZE, max: MAX_SIZE },
		parameter: { min: 0, max: 2 },
	},
} satisfies ShapeDescriptorMap;

const KIND_TAGS: Readonly<Record<SceneKind, number>> = {
	sphere: 1,
	goursat: 2,
	plane: 3,
	crystal: 4,
	finite: 5,
};

const tagKind = (tag: number): SceneKind | undefined => {
	if (tag === KIND_TAGS.sphere) {
		return "sphere";
	}
	if (tag === KIND_TAGS.goursat) {
		return "goursat";
	}
	if (tag === KIND_TAGS.plane) {
		return "plane";
	}
	if (tag === KIND_TAGS.crystal) {
		return "crystal";
	}
	if (tag === KIND_TAGS.finite) {
		return "finite";
	}
	return;
};

const isSceneKind = (kind: unknown): kind is SceneKind =>
	typeof kind === "string" && Object.hasOwn(KIND_TAGS, kind);

const position = (x: number, y: number, z: number): Position =>
	Object.freeze([x, y, z] as const);

const clonePosition = (value: Position): Position =>
	Object.freeze([value[0], value[1], value[2]]);

const materialFields = (
	material: MaterialKind | undefined,
): Readonly<{ material?: MaterialKind }> => {
	if (material === undefined || material === "steel") {
		return {};
	}
	return { material };
};

const rotationFields = (
	rotation: Rotation | undefined,
): Readonly<{ rotation?: Rotation }> => {
	if (rotation === undefined) {
		return {};
	}
	if (!validRotation(rotation)) {
		invalid("invalid rotation");
	}
	const value = normalizeRotation(rotation);
	if (value.every((v, i) => v === IDENTITY_ROTATION[i])) {
		return {};
	}
	return { rotation: value };
};

const sceneObject = ({
	id,
	kind,
	position: objectPosition,
	size,
	parameter,
	material,
	rotation,
}: ReadonlySceneObject): ReadonlySceneObject =>
	Object.freeze({
		id,
		kind,
		position: clonePosition(objectPosition),
		size,
		parameter,
		...materialFields(material),
		...rotationFields(rotation),
	});

class SceneValidationError extends Error {
	constructor(message: string) {
		super(message);
		// biome-ignore lint/security/noSecrets: This is a class name, not a credential.
		this.name = "SceneValidationError";
	}
}

const invalid = (message: string): never => {
	throw new SceneValidationError(message);
};

const validId = (id: number): boolean =>
	Number.isInteger(id) && id >= 1 && id <= 0xff_ff_ff_ff;

const validPosition = (value: Position): boolean =>
	value.length === 3 &&
	Number.isFinite(value[0]) &&
	Number.isFinite(value[1]) &&
	Number.isFinite(value[2]) &&
	value[0] >= MIN_POSITION &&
	value[0] <= MAX_POSITION &&
	value[1] >= MIN_POSITION &&
	value[1] <= MAX_POSITION &&
	value[2] >= MIN_POSITION &&
	value[2] <= MAX_POSITION;

const validateObject = (object: ReadonlySceneObject): void => {
	if (!validId(object.id)) {
		invalid("invalid id");
	}
	if (
		!(
			Number.isFinite(object.position[0]) &&
			Number.isFinite(object.position[1]) &&
			Number.isFinite(object.position[2])
		)
	) {
		invalid("nonfinite");
	}
	if (!validPosition(object.position)) {
		invalid("invalid position");
	}
	if (!isSceneKind(object.kind)) {
		invalid("invalid kind");
	}
	if (object.material !== undefined && !isMaterial(object.material)) {
		invalid("invalid material");
	}
	if (object.rotation !== undefined && !validRotation(object.rotation)) {
		invalid("invalid rotation");
	}
	const descriptor = DESCRIPTORS[object.kind];
	if (
		!Number.isFinite(object.size) ||
		object.size < descriptor.size.min ||
		object.size > descriptor.size.max
	) {
		invalid("invalid size");
	}
	if (
		!Number.isFinite(object.parameter) ||
		(object.kind === "finite" && !Number.isInteger(object.parameter)) ||
		object.parameter < descriptor.parameter.min ||
		object.parameter > descriptor.parameter.max
	) {
		invalid("invalid parameter");
	}
};

const validateScene = (scene: Scene): void => {
	if (scene.length > MAX_OBJECTS) {
		invalid("invalid length");
	}
	const ids = new Set<number>();
	for (const object of scene) {
		validateObject(object);
		if (ids.has(object.id)) {
			invalid("duplicate");
		}
		ids.add(object.id);
	}
};

const DEFAULT_SCENE: Scene = Object.freeze([
	sceneObject({
		id: 1,
		kind: "sphere",
		position: position(-1.65, 1, 0),
		size: 1,
		parameter: 0,
	}),
	sceneObject({
		id: 2,
		kind: "goursat",
		position: position(1.65, 0.75 * Math.sqrt(1 + Math.sqrt(1.5)), 0),
		size: 0.75,
		parameter: 1.5,
	}),
	sceneObject({
		id: 3,
		kind: "plane",
		position: position(0, 0, 0),
		size: 1,
		parameter: 0,
	}),
]);

const packetStride = (posed: boolean): number => {
	if (posed) {
		return POSE_STRIDE;
	}
	return PACKET_STRIDE;
};

const packScene = (scene: Scene): Float64Array => {
	validateScene(scene);
	const rotations = scene.map(
		(object) => rotationFields(object.rotation).rotation,
	);
	const posed = rotations.some((rotation) => rotation !== undefined);
	const start = Number(posed);
	const stride = packetStride(posed);
	const packet = new Float64Array(start + scene.length * stride);
	if (posed) {
		packet[0] = POSE_PACKET_TAG;
	}
	scene.forEach((object, index) => {
		const offset = start + index * stride;
		packet[offset] = object.id;
		packet[offset + 1] = KIND_TAGS[object.kind];
		packet[offset + 2] = object.position[0];
		packet[offset + 3] = object.position[1];
		packet[offset + 4] = object.position[2];
		packet[offset + 5] = object.size;
		packet[offset + 6] = object.parameter;
		packet[offset + 7] = MATERIALS[object.material ?? "steel"].tag;
		if (posed) {
			packet.set(rotations[index] ?? IDENTITY_ROTATION, offset + PACKET_STRIDE);
		}
	});
	return packet;
};

const packetValue = (packet: ArrayLike<number>, index: number): number => {
	const value = packet[index];
	if (value === undefined) {
		throw new SceneValidationError("invalid length");
	}
	return value;
};

const unpackScene = (packet: ArrayLike<number>): Scene => {
	const posed = packet[0] === POSE_PACKET_TAG;
	const start = Number(posed);
	const stride = packetStride(posed);
	if (
		!Number.isInteger(packet.length) ||
		packet.length < start ||
		(packet.length - start) % stride !== 0 ||
		(packet.length - start) / stride > MAX_OBJECTS
	) {
		invalid("invalid length");
	}
	for (let index = 0; index < packet.length; index += 1) {
		if (!Number.isFinite(packetValue(packet, index))) {
			invalid("nonfinite");
		}
	}
	const scene: ReadonlySceneObject[] = [];
	for (let offset: number = start; offset < packet.length; offset += stride) {
		const id = packetValue(packet, offset);
		const kind = tagKind(packetValue(packet, offset + 1));
		if (kind === undefined) {
			throw new SceneValidationError("invalid kind");
		}
		const material = materialKind(packetValue(packet, offset + 7));
		if (material === undefined) {
			throw new SceneValidationError("invalid material");
		}
		const object = sceneObject({
			id,
			kind,
			position: position(
				packetValue(packet, offset + 2),
				packetValue(packet, offset + 3),
				packetValue(packet, offset + 4),
			),
			size: packetValue(packet, offset + 5),
			parameter: packetValue(packet, offset + 6),
			material,
			...(posed && {
				rotation: [
					packetValue(packet, offset + 8),
					packetValue(packet, offset + 9),
					packetValue(packet, offset + 10),
					packetValue(packet, offset + 11),
				] as const,
			}),
		});
		scene.push(object);
	}
	validateScene(scene);
	return Object.freeze(scene);
};

const updateObject = (scene: Scene, id: number, patch: ScenePatch): Scene => {
	validateScene(scene);
	const current = scene.find((object) => object.id === id);
	if (current === undefined) {
		throw new SceneValidationError("unknown id");
	}
	for (const key of Object.keys(patch)) {
		if (
			key !== "position" &&
			key !== "size" &&
			key !== "parameter" &&
			key !== "material" &&
			key !== "rotation"
		) {
			invalid("invalid patch");
		}
	}
	const changed = sceneObject({
		id: current.id,
		kind: current.kind,
		position: patch.position ?? current.position,
		size: patch.size ?? current.size,
		parameter: patch.parameter ?? current.parameter,
		material: patch.material ?? current.material ?? "steel",
		...rotationFields(patch.rotation ?? current.rotation),
	});
	validateObject(changed);
	return Object.freeze(
		scene.map((object) => {
			if (object.id === id) {
				return changed;
			}
			return object;
		}),
	);
};

export {
	DEFAULT_SCENE,
	DESCRIPTORS,
	KIND_TAGS,
	MAX_GOURSAT_PARAMETER,
	MAX_OBJECTS,
	MAX_POSITION,
	MAX_SIZE,
	MIN_GOURSAT_PARAMETER,
	MIN_POSITION,
	MIN_SIZE,
	PACKET_STRIDE,
	POSE_PACKET_TAG,
	POSE_STRIDE,
	SceneValidationError,
	isSceneKind,
	tagKind,
	packScene,
	type NumericRange,
	type Position,
	type ReadonlySceneObject,
	type Scene,
	type SceneKind,
	type SceneObject,
	type ScenePatch,
	type ShapeDescriptor,
	type ShapeDescriptorMap,
	unpackScene,
	updateObject,
};
