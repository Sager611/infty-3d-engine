// biome-ignore-all lint/style/noMagicNumbers: Independent logarithmic speed cases.
// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { MAX_SPEED, MIN_SPEED, stepSpeed } from "./camera.ts";
import {
	listenVelocity,
	renderVelocity,
	speedAt,
	speedPosition,
} from "./velocity.ts";

class Slider extends EventTarget {
	value = "";
	ariaValueText: string | null = null;
	readonly dataset: DOMStringMap = {};
	get valueAsNumber(): number {
		return Number(this.value);
	}
}

test("speed position is logarithmic with exact ends and reversible wheel rates", () => {
	for (const boost of [false, true]) {
		const maximum = MAX_SPEED * (1 + Number(boost));
		expect(speedAt(0, boost)).toBe(MIN_SPEED);
		expect(speedAt(1, boost)).toBe(maximum);
		expect(speedPosition(MIN_SPEED, boost)).toBe(0);
		expect(speedPosition(maximum, boost)).toBe(1);
		expect(speedAt(0.5, boost)).toBeCloseTo(Math.sqrt(MIN_SPEED * maximum), 12);
		for (const speed of [MIN_SPEED, 0.0001, 0.01, 3, maximum]) {
			expect(
				Math.abs(speedAt(speedPosition(speed, boost), boost) / speed - 1),
			).toBeLessThan(1e-12);
		}
	}
	expect(speedPosition(6, true)).toBeGreaterThan(speedPosition(3));
});

test("invalid rates and positions cannot silently corrupt the control", () => {
	for (const position of [-1, 1.01, Number.NaN, Number.POSITIVE_INFINITY]) {
		expect(() => speedAt(position)).toThrow();
	}
	for (const speed of [
		0,
		MIN_SPEED / 2,
		MAX_SPEED + 1,
		Number.NaN,
		Number.POSITIVE_INFINITY,
	]) {
		expect(() => speedPosition(speed)).toThrow();
	}
});

test("readout follows Shift and the applied approach rate without another query; idle restores the set rate", () => {
	const node = new Slider();
	renderVelocity(node, 3);
	expect(node.valueAsNumber).toBeCloseTo(speedPosition(3), 14);
	expect(node.dataset.speed).toBe("3");
	renderVelocity(node, 3, true);
	expect(node.valueAsNumber).toBeCloseTo(speedPosition(6, true), 14);
	expect(node.dataset.speed).toBe("6");
	expect(node.ariaValueText).toContain("units per second");
	let queries = 0;
	const rate = stepSpeed(
		{ forward: 1, side: 0, onlyForward: true, boost: true },
		3,
		0.02,
		() => {
			queries += 1;
			return 0.001;
		},
	);
	renderVelocity(node, rate, true);
	expect(queries).toBe(1);
	expect(Number(node.dataset.speed)).toBe(rate * 2);
	expect(node.valueAsNumber).toBeLessThan(speedPosition(6, true));
	renderVelocity(node, rate);
	expect(node.dataset.speed).toBe(String(rate));
	renderVelocity(node, rate, true);
	expect(node.dataset.speed).toBe(String(rate * 2));
	expect(queries).toBe(1);
	renderVelocity(node, MIN_SPEED);
	expect(node.valueAsNumber).toBe(0);
	renderVelocity(node, 3);
	expect(node.dataset.speed).toBe("3");
	expect(node.dataset.boosted).toBe("false");
});

test("native range decimal normalization stays idempotent and restores a changed position", () => {
	const node = new Slider();
	let value = "";
	let writes = 0;
	Object.defineProperty(node, "value", {
		get: (): string => value,
		set: (next: string): void => {
			value = String(Number(Number(next).toPrecision(15)));
			writes += 1;
		},
	});
	for (const speed of [0.000_999_000_391_810_454, 3, Math.PI]) {
		renderVelocity(node, speed);
		const settled = writes;
		renderVelocity(node, speed);
		expect(writes).toBe(settled);
		node.value = "0";
		renderVelocity(node, speed);
		expect(writes).toBe(settled + 2);
		expect(node.valueAsNumber).toBeCloseTo(speedPosition(speed), 14);
	}
});

test("input changes the base rate, guards errors, and disposes idempotently", () => {
	const node = new Slider();
	const speeds: number[] = [];
	const errors: unknown[] = [];
	let boost = false;
	const dispose = listenVelocity(node, {
		boost: () => boost,
		set: (speed: number): void => {
			speeds.push(speed);
		},
		error: (error: unknown): void => {
			errors.push(error);
		},
	});
	node.value = "0.5";
	node.dispatchEvent(new Event("input"));
	expect(speeds[0]).toBeCloseTo(0.01, 12);
	boost = true;
	node.value = "1";
	node.dispatchEvent(new Event("input"));
	expect(speeds[1]).toBe(MAX_SPEED);
	node.value = "0";
	node.dispatchEvent(new Event("input"));
	expect(speeds[2]).toBe(MIN_SPEED);
	node.value = "invalid";
	node.dispatchEvent(new Event("input"));
	expect(errors).toHaveLength(1);
	expect(speeds).toHaveLength(3);
	dispose();
	dispose();
	node.value = "0.5";
	node.dispatchEvent(new Event("input"));
	expect(speeds).toHaveLength(3);
});

test("left-edge speed control has a continuous vertical range and only endpoint symbols", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const found = { ranges: 0, symbols: "" };
	const parser = new HTMLRewriter()
		.on("#velocity", {
			element(element: HTMLRewriterTypes.Element): void {
				found.ranges += 1;
				expect(element.getAttribute("type")).toBe("range");
				expect(element.getAttribute("aria-label")).toBe("Travel speed");
				expect(element.getAttribute("aria-orientation")).toBe("vertical");
				expect(element.getAttribute("step")).toBe("any");
				expect(element.getAttribute("min")).toBe("0");
				expect(element.getAttribute("max")).toBe("1");
			},
		})
		.on(".velocity > span", {
			text(text: HTMLRewriterTypes.Text): void {
				found.symbols += text.text.trim();
			},
		});
	await parser.transform(new Response(source)).text();
	expect(found).toEqual({ ranges: 1, symbols: "＋−" });
	expect(source).toContain("writing-mode: vertical-lr");
	expect(source).toContain("direction: rtl");
});
