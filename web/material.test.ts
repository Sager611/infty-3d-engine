// biome-ignore-all lint/style/noMagicNumbers: Material IDs and packets are contract fixtures.
// biome-ignore lint/nursery/noUnresolvedImports: Bun builtin.
import { expect, test } from "bun:test";
import { propertyPatch } from "./controls.ts";
import {
	DEFAULT_SCENE,
	packScene,
	unpackScene,
	updateObject,
} from "./scene.ts";
import { DEFAULT_SNAPSHOT, parseSnapshot, serializeSnapshot } from "./state.ts";

test("material selection is per instance, immutable and snapshot-persistent", () => {
	for (const object of DEFAULT_SCENE) {
		const patch = propertyPatch(object, "material", 1);
		expect(patch).toEqual({ material: "brick" });
		const changed = updateObject([object], object.id, patch);
		expect(changed[0]?.material).toBe("brick");
		expect(packScene(changed)[7]).toBe(1);
		expect(unpackScene(packScene(changed))).toEqual(changed);
		expect(packScene([object])[7]).toBe(0);
		const snap = { ...DEFAULT_SNAPSHOT, mode: 3 as const, objects: changed };
		expect(parseSnapshot(serializeSnapshot(snap))).toEqual(snap);
		const steel = updateObject(
			changed,
			object.id,
			propertyPatch(object, "material", 0),
		);
		expect(packScene(steel)[7]).toBe(0);
		expect(unpackScene(packScene(steel))).toEqual(steel);
		expect(() => propertyPatch(object, "material", 3)).toThrow();
	}
});
test("legacy scenes stay valid and unknown material IDs are rejected", () => {
	const packet = packScene(DEFAULT_SCENE);
	expect(unpackScene(packet)).toEqual(DEFAULT_SCENE);
	for (const invalid of [-1, 0.5, 3, Number.NaN, Number.POSITIVE_INFINITY]) {
		const bad = packet.slice();
		bad[7] = invalid;
		expect(() => unpackScene(bad)).toThrow();
	}
});
test("material view is saved without changing query-mode semantics", () => {
	const snap = { ...DEFAULT_SNAPSHOT, mode: 3 as const };
	expect(parseSnapshot(serializeSnapshot(snap))).toEqual(snap);
	expect(() =>
		parseSnapshot(serializeSnapshot(snap).replace(";3;", ";4;")),
	).toThrow();
});
