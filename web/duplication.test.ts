// biome-ignore lint/nursery/noUnresolvedImports: Bun test runtime.
import { expect, test } from "bun:test";

test("scene toolbar offers an accessible duplicate control disabled before selection", async () => {
	const found: string[] = [];
	const source = await Bun.file(
		new URL("../index.html", import.meta.url),
	).text();
	await new HTMLRewriter()
		.on("#scene-tools button", {
			element(element: HTMLRewriterTypes.Element): void {
				if (element.getAttribute("data-scene-action") === "duplicate") {
					found.push(
						element.getAttribute("id") ?? "",
						element.getAttribute("type") ?? "",
						element.getAttribute("aria-label") ?? "",
						element.getAttribute("title") ?? "",
						element.getAttribute("data-tip") ?? "",
						String(element.hasAttribute("disabled")),
					);
				}
			},
		})
		.transform(new Response(source))
		.text();
	expect(found).toEqual([
		"duplicate-objects",
		"button",
		"Duplicate selected objects",
		"Duplicate selected objects",
		"Duplicate selected objects",
		"true",
	]);
});
