// biome-ignore-all lint/style/noMagicNumbers: Locked wire values and bounded fixtures.
// biome-ignore lint/nursery/noUnresolvedImports: Bun builtin.
import { expect, test } from "bun:test";
import { fieldsForObject, propertyPatch } from "./controls.ts";
import { isMaterial, materialKind } from "./material.ts";
import { presetSnapshot } from "./presets.ts";
import {
	isSceneKind,
	packScene,
	tagKind,
	unpackScene,
	updateObject,
} from "./scene.ts";
import { DEFAULT_SNAPSHOT, parseSnapshot, serializeSnapshot } from "./state.ts";

test("WASM embeds the immutable leaf scalar payload once", async (): Promise<void> => {
	const wasm = Buffer.from(
		await Bun.file(
			new URL("../pkg/infty_bg.wasm", import.meta.url),
		).arrayBuffer(),
	);
	const scalar = await Bun.file(
		new URL("../src/heightfield/leaf.bin", import.meta.url),
	).bytes();
	const offset = wasm.indexOf(scalar);
	expect(offset).toBeGreaterThanOrEqual(0);
	expect(wasm.indexOf(scalar, offset + 1)).toBe(-1);
});

test("Finite plane and leaf have stable additive wire tags", (): void => {
	expect(String(tagKind(5))).toBe("finite");
	expect(isSceneKind("finite")).toBe(true);
	expect(String(materialKind(2))).toBe("leaf");
	expect(isMaterial("leaf")).toBe(true);
	expect(tagKind(6)).toBeUndefined();
	expect(materialKind(3)).toBeUndefined();
});

test("Finite leaf packets round-trip every cardinal axis without changing shared peers", (): void => {
	for (const axis of [0, 1, 2]) {
		const packet = [
			7,
			5,
			0,
			0,
			0,
			0.07,
			axis,
			2,
			8,
			5,
			0.5,
			0,
			0,
			0.07,
			axis,
			0,
		];
		const scene = unpackScene(packet);
		expect(String(scene[0]?.kind)).toBe("finite");
		expect(String(scene[0]?.material)).toBe("leaf");
		expect(Array.from(packScene(scene))).toEqual(packet);
		const changed = updateObject(scene, 7, { material: "steel" });
		expect(changed[0]?.material).toBeUndefined();
		expect(String(scene[0]?.material)).toBe("leaf");
		expect(changed[1]).toBe(scene[1]);
	}
});

test("Finite plane axes reject fractional and out-of-range values", (): void => {
	for (const axis of [-1, 0.5, 3]) {
		expect(() => unpackScene([7, 5, 0, 0, 0, 0.07, axis, 0])).toThrow(
			"parameter",
		);
	}
});

test("Leaf preset is immutable, framed, editable and snapshot-persistent", (): void => {
	const before = serializeSnapshot(DEFAULT_SNAPSHOT);
	for (const aspect of [1.5, 390 / 844, 844 / 390]) {
		const state = presetSnapshot("leaf", aspect, 0);
		const object = state.objects[0];
		if (object === undefined) {
			throw new Error("missing leaf");
		}
		expect(String(object.kind)).toBe("finite");
		expect(String(object.material)).toBe("leaf");
		expect(object.size).toBe(0.07);
		expect(state.mode).toBe(3);
		expect(state.camera.target).toEqual(object.position);
		expect(state.camera.distance).toBeGreaterThan(object.size);
		const span =
			(object.size * 1.8 * Math.max(1, 1 / aspect)) / state.camera.distance;
		expect(span).toBeGreaterThan(0.5);
		expect(span).toBeLessThan(0.9);
		expect(
			Object.isFrozen(state.objects) &&
				Object.isFrozen(state.camera) &&
				Object.isFrozen(object) &&
				Object.isFrozen(object.position),
		).toBe(true);
		expect(parseSnapshot(serializeSnapshot(state))).toEqual(state);
		expect(
			fieldsForObject(object).find((f) => f.key === "parameter"),
		).toMatchObject({ min: 0, max: 2, step: 1 });
		const steel = updateObject(
			state.objects,
			object.id,
			propertyPatch(object, "material", 0),
		);
		expect(steel[0]?.material).toBeUndefined();
		expect(String(object.material)).toBe("leaf");
	}
	expect(serializeSnapshot(DEFAULT_SNAPSHOT)).toBe(before);
});
test("Leaf preset and reusable finite carrier are exposed accessibly", async (): Promise<void> => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const found: { preset: string | null; shape: string | null } = {
		preset: null,
		shape: null,
	};
	let active = false;
	const parser = new HTMLRewriter()
		.on("#scene-preset option", {
			element(element: HTMLRewriterTypes.Element): void {
				active = element.getAttribute("value") === "leaf";
			},
			text(t: HTMLRewriterTypes.Text): void {
				if (active && t.text.trim() !== "") {
					found.preset = t.text.trim();
				}
			},
		})
		.on('[data-scene-action="finite"]', {
			text(t: HTMLRewriterTypes.Text): void {
				if (t.text.trim() !== "") {
					found.shape = t.text.trim();
				}
			},
		});
	await parser.transform(new Response(source)).text();
	expect(found.preset).toContain("Leaf");
	expect(found.shape).toContain("Finite plane");
});
test("Old material and geometry tags stay unchanged", (): void => {
	expect(materialKind(0)).toBe("steel");
	expect(materialKind(1)).toBe("brick");
	expect([1, 2, 3, 4].map(tagKind)).toEqual([
		"sphere",
		"goursat",
		"plane",
		"crystal",
	]);
});
