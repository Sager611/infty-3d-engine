// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";

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
