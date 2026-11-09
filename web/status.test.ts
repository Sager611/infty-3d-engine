// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { EMPTY_STATUS, renderedStatus, statusMessage } from "./status.ts";

test("valid frames do not erase action errors or recreate clear state", () => {
	const failure = statusMessage("invalid camera target");
	expect(renderedStatus(failure)).toBe(failure);
	expect(renderedStatus(EMPTY_STATUS)).toBe(EMPTY_STATUS);
});
test("a recovered frame clears render failures; empty messages are canonical", () => {
	expect(renderedStatus(statusMessage("lost surface", "render"))).toBe(
		EMPTY_STATUS,
	);
	expect(statusMessage("", "render")).toBe(EMPTY_STATUS);
});
