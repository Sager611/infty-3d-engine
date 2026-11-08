import { packScene, type SceneObject } from "./scene.ts";

type Disposable = Readonly<{ free(): void }>;
export type SceneCache<T extends Disposable> = Readonly<{
	objects: readonly SceneObject[] | null;
	value: T | null;
}>;

export function emptySceneCache<T extends Disposable>(): SceneCache<T> {
	return Object.freeze({ objects: null, value: null });
}

// Pass returned ownership state onward. A failed creation preserves its input.
export function prepareScene<T extends Disposable>(
	cache: SceneCache<T>,
	objects: readonly SceneObject[],
	create: (packet: Float64Array) => T,
): SceneCache<T> {
	if (cache.objects === objects && cache.value !== null) {
		return cache;
	}
	const value = create(packScene(objects));
	cache.value?.free();
	return Object.freeze({ objects, value });
}

// The callback returns true only after accepting ownership.
export async function acquireResource<T extends Disposable>(
	create: () => Promise<T>,
	accept: (value: T) => boolean,
): Promise<void> {
	const value = await create();
	let accepted = false;
	try {
		accepted = accept(value);
	} finally {
		if (!accepted) {
			value.free();
		}
	}
}

export function releaseScene<T extends Disposable>(
	cache: SceneCache<T>,
): SceneCache<T> {
	cache.value?.free();
	return emptySceneCache<T>();
}
