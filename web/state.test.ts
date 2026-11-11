// biome-ignore-all assist/source/organizeImports: Bun test imports are intentionally grouped.
// biome-ignore-all lint/correctness/useImportExtensions: Bun resolves TypeScript test modules.
// biome-ignore-all lint/nursery/noUnresolvedImports: Bun provides the test module.
// biome-ignore-all lint/style/noMagicNumbers: Contract values are asserted directly.
// biome-ignore-all lint/complexity/noExcessiveLinesPerFunction: Tests are grouped by codec behavior.
import { describe, expect, test } from "bun:test";
import {
	DEFAULT_SNAPSHOT,
	cameraPacket,
	normalizeCamera,
	parseSnapshot,
	serializeSnapshot,
	type Snapshot,
} from "./state";
import { DEFAULT_SCENE } from "./scene";

describe("snapshot codec", () => {
	test("defaults and round trips", () => {
		expect(parseSnapshot("")).toEqual(DEFAULT_SNAPSHOT);
		const snapshot: Snapshot = {
			camera: { yaw: 7, pitch: 0.4, distance: 8, target: [1, 2, 3] },
			mode: 2,
			objects: DEFAULT_SCENE,
		};
		expect(parseSnapshot(serializeSnapshot(snapshot))).toEqual({
			...snapshot,
			camera: normalizeCamera(snapshot.camera),
		});
	});

	test("supports legacy view hashes", () => {
		const snapshot = parseSnapshot("#v=0.1,0.2,6,0.75,1");
		expect(snapshot.mode).toBe(1);
		expect(snapshot.camera).toEqual({
			yaw: 0.1,
			pitch: 0.2,
			distance: 6,
			target: [0, 0.75, 0],
		});
		expect(snapshot.objects[0]).toMatchObject({
			id: 1,
			size: 0.75,
			position: [0, 0.75, 0],
		});
		expect(snapshot.objects.map((object) => object.kind)).toEqual([
			"sphere",
			"plane",
		]);
		expect(snapshot.objects[1]).toEqual(DEFAULT_SCENE[2]);
	});

	test("serializes empty scenes", () => {
		const snapshot = { ...DEFAULT_SNAPSHOT, objects: [] };
		expect(parseSnapshot(serializeSnapshot(snapshot))).toEqual(snapshot);
	});

	test("rejects malformed strict snapshots", () => {
		for (const hash of [
			"#s=2;0,0,1,0,0,0;0;",
			"#s=1;0,2,1,0,0,0;0;",
			"#s=1;0,0,0,0,0,0;0;",
			"#s=1;0,1.5,1,0,0,0;0;",
			"#s=1;0,0,1,1025,0,0;0;",
			"#s=1;0,0,1,0,0,0;4;",
			"#s=1;0,0,1,0,0,0;0;1,1,0,0,0,1,0,3",
		]) {
			expect(() => parseSnapshot(hash)).toThrow();
		}
	});

	test("normalizes camera deterministically", () => {
		const camera = normalizeCamera({
			yaw: Math.PI * 5,
			pitch: 9,
			distance: 0,
			target: [20, -20, 0],
		});
		expect(camera).toEqual({
			yaw: Math.PI,
			pitch: 1.45,
			distance: 0.0001,
			target: [20, -20, 0],
		});
		expect(cameraPacket(camera)).toEqual([Math.PI, 1.45, 0.0001, 20, -20, 0]);
	});
});
