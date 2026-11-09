import { editableTarget } from "./keys.ts";
import { isSceneKind, type SceneKind } from "./scene.ts";

type SceneAction = SceneKind | "delete" | "undo";
function deleteShortcut(
	key: string,
	target: EventTarget | null,
	modified: boolean,
	menuOpen: boolean,
): boolean {
	return (
		(key === "Delete" || key === "Backspace") &&
		!modified &&
		!menuOpen &&
		!editableTarget(target)
	);
}
function listenSceneActions(
	root: EventTarget,
	keyboard: EventTarget,
	menuOpen: () => boolean,
	onAction: (action: SceneAction) => void,
): () => void {
	function click(event: Event): void {
		if (!(event.target instanceof Element)) {
			return;
		}
		const button = event.target.closest<HTMLButtonElement>(
			"[data-scene-action]",
		);
		if (button === null || button.disabled) {
			return;
		}
		const action = button.dataset.sceneAction;
		if (isSceneKind(action) || action === "delete" || action === "undo") {
			onAction(action);
		}
	}
	function keydown(event: Event): void {
		if (!("key" in event) || typeof event.key !== "string") {
			return;
		}
		const modified =
			("ctrlKey" in event && event.ctrlKey === true) ||
			("metaKey" in event && event.metaKey === true) ||
			("altKey" in event && event.altKey === true) ||
			("repeat" in event && event.repeat === true);
		if (!deleteShortcut(event.key, event.target, modified, menuOpen())) {
			return;
		}
		event.preventDefault();
		onAction("delete");
	}
	root.addEventListener("click", click);
	keyboard.addEventListener("keydown", keydown);
	return (): void => {
		root.removeEventListener("click", click);
		keyboard.removeEventListener("keydown", keydown);
	};
}
export { deleteShortcut, listenSceneActions, type SceneAction };
