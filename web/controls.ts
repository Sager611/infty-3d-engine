// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This is a vanilla DOM adapter.
// biome-ignore-all lint/style/noMagicNumbers: Control increments are shared input defaults.
import type { Selection } from "./query.ts";
import {
	DESCRIPTORS,
	MAX_POSITION,
	MIN_POSITION,
	type Position,
	type SceneKind,
	type SceneObject,
	type ScenePatch,
} from "./scene.ts";

const CONTROL_STEP = 0.01;

type ControlKey = "x" | "y" | "z" | "size" | "parameter";

type ControlField = Readonly<{
	key: ControlKey;
	label: string;
	value: number;
	min: number;
	max: number;
	step: number;
}>;

type ControlsState = Readonly<{
	objects: readonly SceneObject[];
	selection: Selection;
	pickAll: boolean;
}>;

type ControlsNodes = Readonly<{
	objectSelect: HTMLSelectElement;
	pickAll: HTMLButtonElement;
	properties: HTMLElement;
	selectionHint: HTMLElement;
}>;

type ControlsEvent =
	| Readonly<{ kind: "select"; id: number | null }>
	| Readonly<{ kind: "pickAll"; value: boolean; force: boolean }>
	| Readonly<{
			kind: "property";
			id: number;
			key: ControlKey;
			value: number;
	  }>;

type ControlsEventHandler = (
	event: ControlsEvent,
	state: ControlsState,
) => void;

type ControlNodeIds = Readonly<{
	objectSelect: string;
	pickAll: string;
	properties: string;
	selectionHint: string;
}>;

const CONTROL_NODE_IDS: ControlNodeIds = Object.freeze({
	objectSelect: "object",
	pickAll: "pick-all",
	properties: "properties",
	selectionHint: "selection-hint",
});

const positionKeys = ["x", "y", "z"] as const;

const positionIndex = (key: string): number => {
	if (key === "x") {
		return 0;
	}
	if (key === "y") {
		return 1;
	}
	if (key === "z") {
		return 2;
	}
	return -1;
};

const field = (values: Omit<ControlField, "step">): ControlField =>
	Object.freeze({
		...values,
		step: CONTROL_STEP,
	});

const positionValue = (
	object: SceneObject,
	key: (typeof positionKeys)[number],
): number => {
	if (key === "x") {
		return object.position[0];
	}
	if (key === "y") {
		return object.position[1];
	}
	return object.position[2];
};

const fieldsForObject = (object: SceneObject): readonly ControlField[] => {
	const descriptor = DESCRIPTORS[object.kind];
	const fields: ControlField[] = positionKeys.map((key) =>
		field({
			key,
			label: key.toUpperCase(),
			value: positionValue(object, key),
			min: MIN_POSITION,
			max: MAX_POSITION,
		}),
	);
	let sizeLabel = "Size";
	if (object.kind === "sphere") {
		sizeLabel = "Radius";
	} else if (object.kind === "plane") {
		sizeLabel = "Grid size";
	}
	fields.push(
		field({
			key: "size",
			label: sizeLabel,
			value: object.size,
			min: descriptor.size.min,
			max: descriptor.size.max,
		}),
	);
	if (object.kind === "goursat") {
		fields.push(
			field({
				key: "parameter",
				label: "Opening β",
				value: object.parameter,
				min: descriptor.parameter.min,
				max: descriptor.parameter.max,
			}),
		);
	}
	return Object.freeze(fields);
};

const unknownProperty = (key: string): never => {
	throw new Error(`unknown property: ${key}`);
};

const propertyPatch = (
	object: SceneObject,
	key: string,
	value: number,
): ScenePatch => {
	if (!Number.isFinite(value)) {
		throw new Error("nonfinite property value");
	}
	const index = positionIndex(key);
	if (index >= 0) {
		const next: [number, number, number] = [
			object.position[0],
			object.position[1],
			object.position[2],
		];
		next[index] = value;
		return Object.freeze({
			position: Object.freeze(next) as Position,
		});
	}
	if (key === "size") {
		return Object.freeze({ size: value });
	}
	if (key === "parameter") {
		return Object.freeze({ parameter: value });
	}
	return unknownProperty(key);
};

const kindLabel = (kind: SceneKind): string =>
	kind.charAt(0).toUpperCase() + kind.slice(1);

const objectLabel = (object: SceneObject): string =>
	`${kindLabel(object.kind)} #${object.id}`;

const replaceObjectOptions = (
	select: HTMLSelectElement,
	objects: readonly SceneObject[],
): void => {
	const options = select.options;
	if (
		options.length === objects.length + 1 &&
		objects.every(
			(object, index) =>
				options[index + 1]?.value === String(object.id) &&
				options[index + 1]?.textContent === objectLabel(object),
		)
	) {
		return;
	}
	const focused = document.activeElement === select;
	const previous = select.value;
	const placeholder = document.createElement("option");
	placeholder.value = "";
	placeholder.textContent = "Select an object";
	select.replaceChildren(placeholder);
	for (const object of objects) {
		const option = document.createElement("option");
		option.value = String(object.id);
		option.textContent = objectLabel(object);
		select.append(option);
	}
	if (focused) {
		select.value = previous;
	}
};

const updateFieldValues = (
	form: HTMLFormElement,
	fields: readonly ControlField[],
): void => {
	for (const controlField of fields) {
		const input = form.elements.namedItem(controlField.key);
		if (input instanceof HTMLInputElement) {
			input.value = String(controlField.value);
			const output = form.querySelector<HTMLOutputElement>(
				`[data-control-value="${controlField.key}"]`,
			);
			if (output !== null) {
				output.value = String(controlField.value);
				output.textContent = String(controlField.value);
				output.title = String(controlField.value);
			}
		}
	}
};

const createPropertyForm = (object: SceneObject): HTMLFormElement => {
	const form = document.createElement("form");
	form.noValidate = true;
	form.dataset.objectId = String(object.id);
	form.dataset.kind = object.kind;
	form.setAttribute("aria-label", `${kindLabel(object.kind)} properties`);
	for (const controlField of fieldsForObject(object)) {
		const label = document.createElement("label");
		const inputId = `property-${object.id}-${controlField.key}`;
		label.htmlFor = inputId;
		label.textContent = controlField.label;
		const output = document.createElement("output");
		output.dataset.controlValue = controlField.key;
		output.value = String(controlField.value);
		output.textContent = String(controlField.value);
		output.title = String(controlField.value);
		label.append(" ", output);
		const input = document.createElement("input");
		input.id = inputId;
		input.name = controlField.key;
		input.type = "range";
		input.min = String(controlField.min);
		input.max = String(controlField.max);
		input.step = String(controlField.step);
		input.value = String(controlField.value);
		input.dataset.controlKey = controlField.key;
		input.title = controlField.label;
		input.setAttribute("aria-label", controlField.label);
		label.append(input);
		form.append(label);
	}
	return form;
};

const clearNoSelection = (nodes: ControlsNodes): void => {
	nodes.objectSelect.value = "";
	nodes.properties.replaceChildren();
	nodes.properties.hidden = true;
	nodes.selectionHint.textContent = "No object selected";
};

const renderProperties = (
	nodes: ControlsNodes,
	object: SceneObject | undefined,
): void => {
	if (object === undefined) {
		clearNoSelection(nodes);
		return;
	}
	nodes.properties.hidden = false;
	const current = nodes.properties.firstElementChild;
	if (
		!(current instanceof HTMLFormElement) ||
		current.dataset.objectId !== String(object.id) ||
		current.dataset.kind !== object.kind
	) {
		nodes.properties.replaceChildren(createPropertyForm(object));
		return;
	}
	updateFieldValues(current, fieldsForObject(object));
};

const renderControls = (nodes: ControlsNodes, state: ControlsState): void => {
	replaceObjectOptions(nodes.objectSelect, state.objects);
	const active = state.objects.find(
		(object) => object.id === state.selection.active,
	);
	let activeValue = "";
	let hint = "No object selected";
	if (active !== undefined) {
		activeValue = String(active.id);
		hint = `Editing ${objectLabel(active)}`;
	}
	nodes.objectSelect.value = activeValue;
	nodes.pickAll.ariaPressed = String(state.pickAll);
	let pickAllLabel = "Next click picks one ray intersection";
	if (state.pickAll) {
		pickAllLabel = "Next click picks all ray intersections";
	}
	nodes.pickAll.title = pickAllLabel;
	nodes.pickAll.dataset.tip = pickAllLabel;
	nodes.pickAll.ariaLabel = pickAllLabel;
	nodes.selectionHint.textContent = hint;
	renderProperties(nodes, active);
};

const findNode = <T extends Element>(
	root: ParentNode,
	id: string,
	type: { new (): T },
): T => {
	const node = root.querySelector(`#${id}`);
	if (!(node instanceof type)) {
		throw new Error(`missing controls node: ${id}`);
	}
	return node;
};

const controlsNodes = (root: ParentNode): ControlsNodes => ({
	objectSelect: findNode(
		root,
		CONTROL_NODE_IDS.objectSelect,
		HTMLSelectElement,
	),
	pickAll: findNode(root, CONTROL_NODE_IDS.pickAll, HTMLButtonElement),
	properties: findNode(root, CONTROL_NODE_IDS.properties, HTMLElement),
	selectionHint: findNode(root, CONTROL_NODE_IDS.selectionHint, HTMLElement),
});

const submit = (event: Event): void => {
	event.preventDefault();
};

const listenControls = (
	nodes: Readonly<{
		objectSelect: EventTarget & { readonly value: string };
		pickAll: EventTarget;
		properties: EventTarget;
	}>,
	getState: () => ControlsState,
	onEvent: ControlsEventHandler,
): (() => void) => {
	const select = (): void => {
		const value = nodes.objectSelect.value;
		let id: number | null = null;
		if (value !== "") {
			id = Number(value);
		}
		onEvent({ kind: "select", id }, getState());
	};
	const pickAll = (event: Event): void => {
		const force = "altKey" in event && event.altKey === true;
		const state = getState();
		onEvent(
			{
				kind: "pickAll",
				value: force || !state.pickAll,
				force,
			},
			state,
		);
	};
	const property = (event: Event): void => {
		const target = event.target;
		if (!(target instanceof HTMLInputElement)) {
			return;
		}
		const state = getState();
		if (state.selection.active === null) {
			return;
		}
		const key = target.dataset.controlKey;
		if (key === undefined) {
			return;
		}
		const value = Number(target.value);
		if (
			key !== "x" &&
			key !== "y" &&
			key !== "z" &&
			key !== "size" &&
			key !== "parameter"
		) {
			return;
		}
		onEvent(
			{
				kind: "property",
				id: state.selection.active,
				key,
				value,
			},
			state,
		);
	};
	nodes.properties.addEventListener("submit", submit);
	nodes.objectSelect.addEventListener("change", select);
	nodes.pickAll.addEventListener("click", pickAll);
	nodes.properties.addEventListener("input", property);
	return (): void => {
		nodes.properties.removeEventListener("submit", submit);
		nodes.objectSelect.removeEventListener("change", select);
		nodes.pickAll.removeEventListener("click", pickAll);
		nodes.properties.removeEventListener("input", property);
	};
};

export {
	CONTROL_NODE_IDS,
	clearNoSelection,
	controlsNodes,
	fieldsForObject,
	listenControls,
	propertyPatch,
	renderControls,
	type ControlField,
	type ControlKey,
	type ControlsEvent,
	type ControlsEventHandler,
	type ControlsNodes,
	type ControlsState,
};
