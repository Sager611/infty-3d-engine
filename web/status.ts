type Status = Readonly<{ text: string; source: "action" | "render" }>;
const EMPTY_STATUS: Status = Object.freeze({ text: "", source: "action" });
function statusMessage(
	text: string,
	source: Status["source"] = "action",
): Status {
	if (text === "") {
		return EMPTY_STATUS;
	}
	return Object.freeze({ text, source });
}
function renderedStatus(current: Status): Status {
	if (current.source === "render") {
		return EMPTY_STATUS;
	}
	return current;
}
export { type Status, EMPTY_STATUS, statusMessage, renderedStatus };
