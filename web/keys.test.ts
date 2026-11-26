// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import type { Movement } from "./camera.ts";
import {
	editableTarget,
	type KeyBindings,
	type KeyController,
	listenKeys,
} from "./keys.ts";
import type { PureCamera } from "./state.ts";

type Fields = Readonly<{
	type?: "keydown" | "keyup";
	focus?: EventTarget;
	repeat?: boolean;
	ctrlKey?: boolean;
	altKey?: boolean;
	metaKey?: boolean;
	shiftKey?: boolean;
}>;
class Key extends Event {
	readonly key: string;
	readonly focus: EventTarget | undefined;
	constructor(key: string, fields: Fields = {}) {
		super(fields.type ?? "keydown", { cancelable: true });
		this.key = key;
		const { type: _type, focus, ...flags } = fields;
		this.focus = focus;
		Object.assign(this, flags);
	}
	override get target(): EventTarget | null {
		return this.focus ?? super.target;
	}
}
interface State {
	camera: PureCamera;
	moves: Movement[];
	speeds: number[];
	errors: unknown[];
	resets: number;
	frames: number;
	hidden: boolean;
	blocked: boolean;
}
type Fixture = Readonly<{
	keyboard: EventTarget;
	visibility: EventTarget;
	state: State;
	controller: KeyController;
}>;
function fixture(): Fixture {
	const keyboard = new EventTarget();
	const visibility = new EventTarget();
	const state: State = {
		camera: { yaw: 0, pitch: 0, distance: 5, target: [0, 0, 0] },
		moves: [],
		speeds: [],
		errors: [],
		resets: 0,
		frames: 0,
		hidden: false,
		blocked: false,
	};
	const bindings: KeyBindings = {
		keyboard,
		visibility,
		hidden: () => state.hidden,
		blocked: () => state.blocked,
		getCamera: () => state.camera,
		setCamera(camera: PureCamera): void {
			state.camera = camera;
		},
		movement(value: Movement): void {
			state.moves.push(value);
		},
		speed(value: number): void {
			state.speeds.push(value);
		},
		reset(): void {
			state.resets += 1;
		},
		frame(): void {
			state.frames += 1;
		},
		onError(error: unknown): void {
			state.errors.push(error);
		},
	};
	return { keyboard, visibility, state, controller: listenKeys(bindings) };
}
function send(f: Fixture, key: string, fields: Fields = {}): boolean {
	return !f.keyboard.dispatchEvent(new Key(key, fields));
}

test("held keys emit absolute movement; diagonals/opposites never enable W-only easing", () => {
	const f = fixture();
	expect(send(f, "w")).toBe(true);
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 0,
	});
	const count = f.state.moves.length;
	send(f, "w", { repeat: true });
	send(f, "w");
	expect(f.state.moves.length).toBe(count);
	send(f, "d");
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 1,
	});
	send(f, "a");
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 0,
	});
	send(f, "s");
	expect(f.state.moves.at(-1)).toEqual({
		forward: 0,
		side: 0,
	});
	f.controller.stop();
	send(f, "S");
	expect(f.state.moves.at(-1)).toEqual({
		forward: -1,
		side: 0,
	});
	f.controller.dispose();
});

test("blur, hidden pages, editable focus, menus and cleanup release held keys", () => {
	const f = fixture();
	send(f, "w");
	f.keyboard.dispatchEvent(new Event("blur"));
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	send(f, "w", { repeat: true });
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	send(f, "w");
	f.state.hidden = true;
	f.visibility.dispatchEvent(new Event("visibilitychange"));
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	expect(send(f, "w")).toBe(false);
	f.state.hidden = false;
	const field = Object.assign(new EventTarget(), { tagName: "INPUT" });
	expect(editableTarget(field)).toBe(true);
	send(f, "w");
	send(f, "w", { type: "keyup", focus: field });
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	expect(send(f, "w", { focus: field })).toBe(false);
	f.state.blocked = true;
	expect(send(f, "w")).toBe(false);
	f.state.blocked = false;
	send(f, "w");
	f.controller.dispose();
	const count = f.state.moves.length;
	f.controller.dispose();
	send(f, "w");
	expect(f.state.moves.length).toBe(count);
});

test("Shift release cannot restart movement while hidden, blocked or editing", () => {
	for (const guard of ["hidden", "blocked"] as const) {
		const f = fixture();
		send(f, "w");
		send(f, "Shift", { shiftKey: true });
		f.state[guard] = true;
		send(f, "Shift", { type: "keyup", shiftKey: false });
		expect(f.state.moves.at(-1)?.forward).toBe(0);
		expect(f.state.moves.at(-1)?.boost).not.toBe(true);
		f.state[guard] = false;
		send(f, "w", { repeat: true });
		expect(f.state.moves.at(-1)?.forward).toBe(0);
		f.controller.dispose();
	}
	const f = fixture();
	send(f, "w");
	send(f, "Shift", { shiftKey: true });
	const focus = Object.assign(new EventTarget(), { tagName: "INPUT" });
	send(f, "Shift", { type: "keyup", focus });
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	f.controller.dispose();
});

test("modifiers preserve browser shortcuts; look, speed, reset and frame stay guarded", () => {
	const f = fixture();
	for (const fields of [
		{ ctrlKey: true },
		{ altKey: true },
		{ metaKey: true },
	]) {
		expect(send(f, "w", fields)).toBe(false);
	}
	send(f, "ArrowRight");
	const { camera } = f.state;
	expect(camera.yaw).toBeLessThan(0);
	expect(
		camera.target[0] +
			camera.distance * Math.cos(camera.pitch) * Math.sin(camera.yaw),
	).toBeCloseTo(0);
	send(f, "+");
	send(f, "-");
	expect(f.state.speeds[0]).toBeLessThan(0);
	expect(f.state.speeds[1]).toBeGreaterThan(0);
	send(f, "w");
	send(f, "r");
	send(f, "f");
	expect(f.state.resets).toBe(1);
	expect(f.state.frames).toBe(1);
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	f.controller.dispose();
});

test("held Shift doubles movement temporarily, including mid-move and both Shift keys", () => {
	const f = fixture();
	send(f, "w");
	send(f, "Shift", { shiftKey: true });
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 0,
		boost: true,
	});
	const count = f.state.moves.length;
	send(f, "Shift", { shiftKey: true, repeat: true });
	expect(f.state.moves).toHaveLength(count);
	send(f, "Shift", { type: "keyup", shiftKey: true });
	expect(f.state.moves).toHaveLength(count);
	send(f, "Shift", { type: "keyup", shiftKey: false });
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 0,
	});
	send(f, "d", { shiftKey: true });
	expect(f.state.moves.at(-1)).toEqual({
		forward: 1,
		side: 1,
		boost: true,
	});
	f.keyboard.dispatchEvent(new Event("blur"));
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	expect(f.state.moves.at(-1)?.boost).not.toBe(true);
	send(f, "w", { repeat: true, shiftKey: true });
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	send(f, "w");
	expect(f.state.moves.at(-1)?.boost).not.toBe(true);
	expect(f.state.speeds).toHaveLength(0);
	expect(f.state.errors).toHaveLength(0);
	f.controller.dispose();
});

const MAX_CAMERA_TARGET = 1024;
test("numerical look errors are reported and clear movement", () => {
	const f = fixture();
	f.state.camera = { ...f.state.camera, target: [MAX_CAMERA_TARGET, 0, 0] };
	send(f, "w");
	send(f, "ArrowRight");
	expect(f.state.errors).toHaveLength(1);
	expect(f.state.moves.at(-1)?.forward).toBe(0);
	f.controller.dispose();
});
