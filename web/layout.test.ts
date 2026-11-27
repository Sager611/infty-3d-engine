// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";

test("Materials is an additional accessible view without replacing diagnostics", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const modes: string[] = [];
	const labels: string[] = [];
	const parser = new HTMLRewriter().on("#mode option", {
		element(element: HTMLRewriterTypes.Element): void {
			modes.push(element.getAttribute("value") ?? "");
		},
		text(text: HTMLRewriterTypes.Text): void {
			if (text.text.trim() !== "") {
				labels.push(text.text.trim());
			}
		},
	});
	await parser.transform(new Response(source)).text();
	expect(modes).toEqual(["0", "1", "2", "3"]);
	expect(labels).toEqual(["UV", "Gap", "Detail", "Materials"]);
});

test("texh displacement toggle starts on inside the scene controls", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const toggles: unknown[][] = [];
	const parser = new HTMLRewriter().on("#controls label > #texh", {
		element(element: HTMLRewriterTypes.Element): void {
			toggles.push([
				element.getAttribute("type"),
				element.hasAttribute("checked"),
				element.getAttribute("title"),
			]);
		},
	});
	await parser.transform(new Response(source)).text();
	expect(toggles).toEqual([["checkbox", true, "Heightmap displacement"]]);
});

test("property forms have a non-form host and their own grid", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	let host = "";
	const parser = new HTMLRewriter().on("#properties", {
		element(element: HTMLRewriterTypes.Element): void {
			host = element.tagName;
		},
	});
	await parser.transform(new Response(source)).text();
	expect(host).toBe("div");
	expect(source).toContain(".properties form {");
	expect(source).toContain("#probe:not([open]) {");
});

test("mobile scene tools leave room for the compact probe", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	expect(source.replaceAll(/\s+/g, " ")).toContain(
		"@media (max-width: 600px) { #probe:not([open]) { justify-self: end; } }",
	);
});

test("scene loader exposes all presets inside the accessible controls", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const options: string[] = [];
	const found: { label: string | null } = { label: null };
	const parser = new HTMLRewriter()
		.on("#scene-controls select#scene-preset", {
			element(element: HTMLRewriterTypes.Element): void {
				found.label = element.getAttribute("aria-label");
			},
		})
		.on("#scene-preset option", {
			element(element: HTMLRewriterTypes.Element): void {
				options.push(element.getAttribute("value") ?? "");
			},
		});
	await parser.transform(new Response(source)).text();
	expect(found.label).toBe("Load scene");
	expect(options).toEqual(["", "default", "showcase", "leaf", "brick"]);
});

test("scene controls stay inside a collapsed, accessible disclosure", async () => {
	const source: string = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const found: { panels: number; toggles: number; controls: number } = {
		panels: 0,
		toggles: 0,
		controls: 0,
	};
	const parser: HTMLRewriter = new HTMLRewriter()
		.on("details#scene-controls", {
			element(element: HTMLRewriterTypes.Element): void {
				found.panels += 1;
				expect(element.hasAttribute("open")).toBe(false);
			},
		})
		.on('details#scene-controls > summary[aria-label="Scene controls"]', {
			element(): void {
				found.toggles += 1;
			},
		})
		.on("details#scene-controls section#controls", {
			element(): void {
				found.controls += 1;
			},
		});
	await parser.transform(new Response(source)).text();
	expect(found).toEqual({ panels: 1, toggles: 1, controls: 1 });
});

test("a loading screen names its step until the first frame", async () => {
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	const screens: unknown[] = [];
	const parser = new HTMLRewriter().on("body > #loading", {
		element(element: HTMLRewriterTypes.Element): void {
			screens.push(
				element.getAttribute("role"),
				element.getAttribute("aria-busy"),
			);
		},
		text(text: HTMLRewriterTypes.Text): void {
			if (text.text.trim() !== "") {
				screens.push(text.text.trim());
			}
		},
	});
	await parser.transform(new Response(source)).text();
	expect(screens).toEqual(["status", "true", "Loading engine"]);
});
