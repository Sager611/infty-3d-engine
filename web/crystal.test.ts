// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Tests are not Qwik components.
// biome-ignore-all lint/style/noMagicNumbers: Exact geometry and packet contract values.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides the virtual test module.
import { expect, test } from "bun:test";
import { objectRadius } from "./camera.ts";
import { addInstance, createEditor } from "./editing.ts";
import { decodeQueries } from "./query.ts";
import {
	DESCRIPTORS,
	packScene,
	type SceneObject,
	unpackScene,
} from "./scene.ts";
import { DEFAULT_SNAPSHOT, parseSnapshot, serializeSnapshot } from "./state.ts";

const crystal: SceneObject = {
	id: 7,
	kind: "crystal",
	position: [0, 1, 0],
	size: 2,
	parameter: 0,
};

test("crystal packets and snapshots retain tag four and zero parameters", () => {
	const packet = packScene([crystal]);
	expect(packet).toEqual(new Float64Array([7, 4, 0, 1, 0, 2, 0, 0]));
	expect(unpackScene(packet)).toEqual([crystal]);
	const snapshot = { ...DEFAULT_SNAPSHOT, objects: [crystal] };
	expect(parseSnapshot(serializeSnapshot(snapshot))).toEqual(snapshot);
	expect(DESCRIPTORS.crystal.parameter).toEqual({ min: 0, max: 0 });
	expect(() => packScene([{ ...crystal, parameter: 1 }])).toThrow(
		"invalid parameter",
	);
});

test("crystal framing uses its certified enclosing radius", () => {
	expect(objectRadius(crystal)).toBe(3.5);
});

test("crystal query packets retain ordinary zero/one status flags", () => {
	const packet = new Float64Array(24);
	packet.set([7, 4, 1, 2]);
	expect(decodeQueries(packet)[0]?.kind).toBe("crystal");
	packet[22] = 2;
	expect(() => decodeQueries(packet)).toThrow();
});

test("the editor spawns a selectable crystal with valid metadata", () => {
	const editor = addInstance(
		createEditor([]),
		"crystal",
		DEFAULT_SNAPSHOT.camera,
	);
	expect(editor.objects[0]?.kind).toBe("crystal");
	expect(editor.selection.active).toBe(editor.objects[0]?.id ?? null);
	expect(unpackScene(packScene(editor.objects))).toEqual(editor.objects);
});
