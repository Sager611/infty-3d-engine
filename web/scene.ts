// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
// biome-ignore-all lint/style/noMagicNumbers: Packet indices and locked defaults are contract values.
type Position = readonly [number, number, number];
type SceneKind = "sphere" | "goursat" | "plane";

type ReadonlySceneObject = Readonly<{
	id: number;
	kind: SceneKind;
	position: Position;
	size: number;
	parameter: number;
}>;

type SceneObject = ReadonlySceneObject;
type Scene = readonly ReadonlySceneObject[];
type ScenePatch = Readonly<{
	position?: Position;
	size?: number;
	parameter?: number;
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
} satisfies ShapeDescriptorMap;

const KIND_TAGS: Readonly<Record<SceneKind, number>> = {
	sphere: 1,
	goursat: 2,
	plane: 3,
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
	return;
};

const position = (x: number, y: number, z: number): Position =>
	Object.freeze([x, y, z] as const);

const clonePosition = (value: Position): Position =>
	Object.freeze([value[0], value[1], value[2]]);

const sceneObject = ({
	id,
	kind,
	position: objectPosition,
	size,
	parameter,
}: ReadonlySceneObject): ReadonlySceneObject =>
	Object.freeze({
		id,
		kind,
		position: clonePosition(objectPosition),
		size,
		parameter,
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
	const descriptor = DESCRIPTORS[object.kind];
	if (descriptor === undefined) {
		invalid("invalid kind");
	}
	if (
		!Number.isFinite(object.size) ||
		object.size < descriptor.size.min ||
		object.size > descriptor.size.max
	) {
		invalid("invalid size");
	}
	if (
		!Number.isFinite(object.parameter) ||
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

const packScene = (scene: Scene): Float64Array => {
	validateScene(scene);
	const packet = new Float64Array(scene.length * PACKET_STRIDE);
	scene.forEach((object, index) => {
		const offset = index * PACKET_STRIDE;
		packet[offset] = object.id;
		packet[offset + 1] = KIND_TAGS[object.kind];
		packet[offset + 2] = object.position[0];
		packet[offset + 3] = object.position[1];
		packet[offset + 4] = object.position[2];
		packet[offset + 5] = object.size;
		packet[offset + 6] = object.parameter;
		packet[offset + 7] = 0;
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
	if (
		!Number.isInteger(packet.length) ||
		packet.length % PACKET_STRIDE !== 0 ||
		packet.length / PACKET_STRIDE > MAX_OBJECTS
	) {
		invalid("invalid length");
	}
	for (let index = 0; index < packet.length; index += 1) {
		if (!Number.isFinite(packetValue(packet, index))) {
			invalid("nonfinite");
		}
	}
	const scene: ReadonlySceneObject[] = [];
	const ids = new Set<number>();
	for (let offset = 0; offset < packet.length; offset += PACKET_STRIDE) {
		const id = packetValue(packet, offset);
		const kind = tagKind(packetValue(packet, offset + 1));
		if (!validId(id)) {
			invalid("invalid id");
		}
		if (kind === undefined) {
			throw new SceneValidationError("invalid kind");
		}
		if (packetValue(packet, offset + 7) !== 0) {
			invalid("invalid padding");
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
		});
		validateObject(object);
		if (ids.has(id)) {
			invalid("duplicate");
		}
		ids.add(id);
		scene.push(object);
	}
	return Object.freeze(scene);
};

const updateObject = (scene: Scene, id: number, patch: ScenePatch): Scene => {
	validateScene(scene);
	const current = scene.find((object) => object.id === id);
	if (current === undefined) {
		throw new SceneValidationError("unknown id");
	}
	for (const key of Object.keys(patch)) {
		if (key !== "position" && key !== "size" && key !== "parameter") {
			invalid("invalid patch");
		}
	}
	const changed = sceneObject({
		id: current.id,
		kind: current.kind,
		position: patch.position ?? current.position,
		size: patch.size ?? current.size,
		parameter: patch.parameter ?? current.parameter,
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
	SceneValidationError,
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
