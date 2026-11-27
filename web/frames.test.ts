// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import {
	createFrames,
	type FrameDriver,
	type Frames,
	presented,
} from "./frames.ts";

interface Fixture {
	frames: Frames;
	queue: Map<number, (now: number) => void>;
	cancelled: number[];
	drawn: number[];
	visible: boolean;
	moving: boolean;
	fail: boolean;
}
function fixture(): Fixture {
	let id = 0;
	const state: Omit<Fixture, "frames"> = {
		queue: new Map<number, (now: number) => void>(),
		cancelled: [],
		drawn: [],
		visible: true,
		moving: false,
		fail: false,
	};
	const driver: FrameDriver = {
		request(tick: (now: number) => void): number {
			id += 1;
			state.queue.set(id, tick);
			return id;
		},
		cancel(value: number): void {
			state.cancelled.push(value);
			state.queue.delete(value);
		},
		visible: () => state.visible,
		moving: () => state.moving,
		draw(now: number): void {
			if (state.fail) {
				throw new Error("draw failed");
			}
			state.drawn.push(now);
		},
	};
	// Keep the getters' state identical to the returned fixture.
	return Object.assign(state, { frames: createFrames(driver) });
}
function fire(f: Fixture): void {
	const next = f.queue.entries().next().value;
	if (next === undefined) {
		throw new Error("No requested frame");
	}
	const [id, tick] = next;
	f.queue.delete(id);
	tick(id);
}

test("invalidation coalesces and idle scenes never request another frame", () => {
	const f = fixture();
	f.frames.invalidate();
	f.frames.invalidate();
	expect(f.queue.size).toBe(1);
	fire(f);
	expect(f.drawn).toHaveLength(1);
	expect(f.queue.size).toBe(0);
});
test("movement schedules one successor; release and hidden pages stop the loop", () => {
	const f = fixture();
	f.moving = true;
	f.frames.invalidate();
	fire(f);
	expect(f.queue.size).toBe(1);
	f.moving = false;
	fire(f);
	expect(f.queue.size).toBe(0);
	f.frames.invalidate();
	f.visible = false;
	f.frames.visibility();
	expect(f.queue.size).toBe(0);
	f.frames.invalidate();
	expect(f.queue.size).toBe(0);
	f.visible = true;
	f.frames.visibility();
	expect(f.queue.size).toBe(1);
	fire(f);
	expect(f.queue.size).toBe(0);
});
test("disposal cancels once and late callbacks cannot draw", () => {
	const f = fixture();
	f.frames.invalidate();
	const late = f.queue.values().next().value;
	f.frames.dispose();
	f.frames.dispose();
	f.frames.invalidate();
	expect(f.cancelled).toHaveLength(1);
	late?.(0);
	expect(f.drawn).toHaveLength(0);
	expect(f.queue.size).toBe(0);
});
test("a draw failure propagates without starting a retry loop", () => {
	const f = fixture();
	f.moving = true;
	f.fail = true;
	f.frames.invalidate();
	expect(() => fire(f)).toThrow("draw failed");
	expect(f.queue.size).toBe(0);
});

test("presented waits for the frame after the next one", async () => {
	const queue: ((now: number) => void)[] = [];
	let shown = false;
	const done = (async (): Promise<void> => {
		await presented((tick) => queue.push(tick));
		shown = true;
	})();
	for (const tick of queue.splice(0)) {
		tick(0);
	}
	await Promise.resolve();
	expect(shown).toBe(false);
	for (const tick of queue.splice(0)) {
		tick(0);
	}
	await done;
	expect(shown).toBe(true);
});
