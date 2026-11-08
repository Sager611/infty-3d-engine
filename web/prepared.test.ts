// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this module at runtime.
import { expect, test } from "bun:test";
import {
	acquireResource,
	emptySceneCache,
	prepareScene,
	releaseScene,
} from "./prepared.ts";
import { DEFAULT_SCENE, updateObject } from "./scene.ts";

type Resource = Readonly<{ id: number; free(): void }>;
const OBJECT_STRIDE = 8;

test("unchanged object arrays reuse resources; replacement and release free once", () => {
	let created = 0;
	const freed: number[] = [];
	function create(packet: Float64Array): Resource {
		expect(packet.length).toBe(DEFAULT_SCENE.length * OBJECT_STRIDE);
		created += 1;
		const id = created;
		return {
			id,
			free(): void {
				freed.push(id);
			},
		};
	}
	let state = prepareScene(emptySceneCache<Resource>(), DEFAULT_SCENE, create);
	const first = state.value;
	state = prepareScene(state, DEFAULT_SCENE, create);
	expect(state.value).toBe(first);
	expect(created).toBe(1);
	expect(freed).toEqual([]);
	const changed = updateObject(DEFAULT_SCENE, 1, { size: 0.5 });
	state = prepareScene(state, changed, create);
	expect(state.value?.id).toBe(2);
	expect(freed).toEqual([1]);
	state = releaseScene(state);
	state = releaseScene(state);
	expect(state.value).toBeNull();
	expect(freed).toEqual([1, 2]);
});

test("failed preparation leaves the previous resource alive", () => {
	let freed = false;
	function create(): Resource {
		return {
			id: 1,
			free(): void {
				freed = true;
			},
		};
	}
	const before = prepareScene(
		emptySceneCache<Resource>(),
		DEFAULT_SCENE,
		create,
	);
	const changed = updateObject(DEFAULT_SCENE, 1, { size: 0.5 });
	function fail(): Resource {
		throw new Error("prepare failed");
	}
	expect(() => prepareScene(before, changed, fail)).toThrow("prepare failed");
	expect(freed).toBe(false);
	expect(before.objects).toBe(DEFAULT_SCENE);
	expect(before.value?.id).toBe(1);
	releaseScene(before);
});

test("deferred resources cannot outlive disposal or rejected ownership", async () => {
	let disposed = false;
	let freed = 0;
	let accepted: Resource | null = null;
	const resource: Resource = {
		id: 1,
		free(): void {
			freed += 1;
		},
	};
	async function create(): Promise<Resource> {
		await Promise.resolve();
		disposed = true;
		return resource;
	}
	function accept(value: Resource): boolean {
		if (disposed) {
			return false;
		}
		accepted = value;
		return true;
	}
	await acquireResource(create, accept);
	expect(freed).toBe(1);
	expect(accepted).toBeNull();
});

test("accepted resources stay owned; failed acceptance releases once", async () => {
	let freed = 0;
	const resource: Resource = {
		id: 1,
		free(): void {
			freed += 1;
		},
	};
	function create(): Promise<Resource> {
		return Promise.resolve(resource);
	}
	function accept(): boolean {
		return true;
	}
	await acquireResource(create, accept);
	expect(freed).toBe(0);
	function fail(): boolean {
		throw new Error("accept failed");
	}
	await expect(acquireResource(create, fail)).rejects.toThrow("accept failed");
	expect(freed).toBe(1);
});
