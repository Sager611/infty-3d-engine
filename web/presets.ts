import { frameCamera } from "./camera.ts";
import {
	DEFAULT_SCENE,
	type Scene,
	type SceneKind,
	type SceneObject,
} from "./scene.ts";
import { DEFAULT_SNAPSHOT, type Snapshot } from "./state.ts";
import { MATERIAL_MODE } from "./view.ts";

const PANEL_FRAME = 0.4;
const LEAF_SIZE = 0.07;
const BRICK_SIZE = 0.42;

function panel(material: "leaf" | "brick", size: number): Scene {
	return Object.freeze([
		Object.freeze({
			id: 1,
			kind: "finite",
			position: Object.freeze([0, 1, 0] as const),
			size,
			parameter: 0,
			material,
		}),
	]);
}

const PANELS = {
	leaf: { objects: panel("leaf", LEAF_SIZE), yaw: 0.28, pitch: 0.32 },
	brick: { objects: panel("brick", BRICK_SIZE), yaw: 0.52, pitch: 0.16 },
} as const;

const SIDE = 16;
const LAYER: number = SIDE * SIDE;
const COUNT: number = LAYER * SIDE;
const CENTER: number = (SIDE - 1) / 2;
const SPACING = 0.8;
const HEIGHT = 0.26;
const SIZE = 0.14;
const BETA = 1.5;
const YAW = 0.65;
const PITCH = 0.38;
const MIX_PERIOD = 2;

function showcase(): Scene {
	const objects: SceneObject[] = [];
	for (let index = 0; index < COUNT; index += 1) {
		let kind: SceneKind = "sphere";
		let parameter = 0;
		if (index % MIX_PERIOD !== 0) {
			kind = "goursat";
			parameter = BETA;
		}
		objects.push(
			Object.freeze({
				id: index + 1,
				kind,
				position: Object.freeze([
					((index % SIDE) - CENTER) * SPACING,
					(Math.floor(index / SIDE) % SIDE) * SPACING + HEIGHT,
					(Math.floor(index / LAYER) - CENTER) * SPACING,
				] as const),
				size: SIZE,
				parameter,
			}),
		);
	}
	objects.push(
		Object.freeze({
			id: COUNT + 1,
			kind: "plane",
			position: Object.freeze([0, 0, 0] as const),
			size: 1,
			parameter: 0,
		}),
	);
	return Object.freeze(objects);
}

export function presetSnapshot(
	name: string,
	aspect: number,
	mode: Snapshot["mode"],
): Snapshot {
	let objects = DEFAULT_SCENE;
	let camera = DEFAULT_SNAPSHOT.camera;
	let viewMode = mode;
	if (name === "showcase") {
		objects = showcase();
		camera = { ...camera, yaw: YAW, pitch: PITCH };
	} else if (name === "leaf" || name === "brick") {
		const preset = PANELS[name];
		objects = preset.objects;
		camera = { ...camera, yaw: preset.yaw, pitch: preset.pitch };
		viewMode = MATERIAL_MODE;
	} else if (name !== "default") {
		throw new Error("Unknown scene preset");
	}
	camera = frameCamera(camera, objects, [], aspect);
	if (name === "leaf" || name === "brick") {
		camera = Object.freeze({
			...camera,
			distance: camera.distance * PANEL_FRAME,
		});
	}
	return Object.freeze({
		objects,
		mode: viewMode,
		camera,
	});
}
