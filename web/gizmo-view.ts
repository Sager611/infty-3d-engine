// biome-ignore-all lint/style/noMagicNumbers: Fixed SVG handle dimensions and RGB axis colors.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Owned vanilla DOM, not Qwik.
import {
	AXIS_IDS,
	type Handle,
	type HandleGeometry,
	layoutGizmo,
	makeProjection,
	type Pixel,
} from "./gizmo.ts";
import {
	type GizmoContext,
	type GizmoInput,
	listenGizmo,
} from "./gizmo-input.ts";
import type { SceneObject, ScenePatch } from "./scene.ts";
import type { PureCamera } from "./state.ts";

type WidgetBindings = Readonly<{
	canvas: HTMLCanvasElement;
	camera: () => PureCamera;
	object: () => SceneObject | undefined;
	blocked: () => boolean;
	start: () => void;
	change: (id: number, patch: ScenePatch) => void;
	error: (error: unknown) => void;
}>;
type GizmoWidget = Pick<GizmoInput, "dragging" | "cancel" | "dispose"> &
	Readonly<{ update: () => void }>;
type Control = Readonly<{
	handle: Handle;
	group: SVGGElement;
	line: SVGPathElement;
	halo: SVGPathElement;
	hit: SVGPathElement;
	head: SVGPathElement;
	label: SVGTextElement;
}>;
const AXIS_NAMES = ["X", "Y", "Z"] as const;
const COLORS = ["#db535a", "#249862", "#4084df"] as const;
const ACTIONS: Readonly<Record<Handle["mode"], string>> = {
	move: "Move",
	rotate: "Rotate",
};
const STEPS: Readonly<Record<Handle["mode"], string>> = {
	move: "0.001",
	rotate: "1°",
};
const node = <K extends keyof SVGElementTagNameMap>(
	document: Document,
	tag: K,
): SVGElementTagNameMap[K] =>
	document.createElementNS("http://www.w3.org/2000/svg", tag);
const keyOf = (handle: Handle): string => handle.mode + handle.axis;
const createControl = (document: Document, handle: Handle): Control => {
	const group = node(document, "g");
	group.classList.add("gizmo-handle");
	group.dataset.gizmoMode = handle.mode;
	group.dataset.gizmoAxis = String(handle.axis);
	group.style.setProperty("--axis-color", COLORS[handle.axis]);
	group.setAttribute("role", "button");
	group.setAttribute("tabindex", "0");
	const name = `${ACTIONS[handle.mode]} ${AXIS_NAMES[handle.axis]}`;
	group.setAttribute("aria-label", name);
	const title = node(document, "title");
	title.textContent = `${name} · Drag · Shift 2× · Arrows/Enter ${STEPS[handle.mode]} · Esc cancels`;
	const halo = node(document, "path");
	halo.classList.add("gizmo-halo");
	const line = node(document, "path");
	line.classList.add("gizmo-line");
	const hit = node(document, "path");
	hit.classList.add("gizmo-hit");
	const head = node(document, "path");
	head.classList.add("gizmo-head");
	const label = node(document, "text");
	if (handle.mode === "move") {
		label.textContent = AXIS_NAMES[handle.axis];
	}
	group.append(title, halo, line, head, label, hit);
	return { handle, group, line, halo, hit, head, label };
};
const arrow = (control: Control, points: readonly Pixel[]): void => {
	const first = points[0];
	const last = points.at(-1);
	if (first === undefined || last === undefined) {
		return;
	}
	const dx = last[0] - first[0];
	const dy = last[1] - first[1];
	const length = Math.hypot(dx, dy);
	if (length < 1) {
		control.head.setAttribute(
			"d",
			`M ${last[0] - 4} ${last[1]} a 4 4 0 1 0 8 0 a 4 4 0 1 0 -8 0`,
		);
		control.label.setAttribute("x", String(last[0] + 12));
		control.label.setAttribute("y", String(last[1] + 12));
		return;
	}
	const x = dx / length;
	const y = dy / length;
	control.head.setAttribute(
		"d",
		`M ${last[0]} ${last[1]} L ${last[0] - x * 10 + y * 4.5} ${last[1] - y * 10 - x * 4.5} L ${last[0] - x * 10 - y * 4.5} ${last[1] - y * 10 + x * 4.5} Z`,
	);
	control.label.setAttribute("x", String(last[0] + x * 13));
	control.label.setAttribute("y", String(last[1] + y * 13));
};
const drawControl = (control: Control, geometry: HandleGeometry): void => {
	const path = `M ${geometry.points.map((point) => point.map((value) => value.toFixed(2)).join(" ")).join(" L ")}`;
	for (const element of [control.line, control.halo, control.hit]) {
		element.setAttribute("d", path);
	}
	if (control.handle.mode === "move") {
		arrow(control, geometry.points);
	}
};
const controlAt = (
	root: SVGSVGElement,
	target: EventTarget | null,
): SVGGElement | null => {
	if (!(target instanceof Element)) {
		return null;
	}
	const group = target.closest<SVGGElement>("g[data-gizmo-mode]");
	if (group !== null && root.contains(group)) {
		return group;
	}
	return null;
};

const createRoot = (canvas: HTMLCanvasElement): SVGSVGElement => {
	const document = canvas.ownerDocument;
	const root = node(document, "svg");
	root.classList.add("gizmo");
	root.setAttribute("role", "group");
	root.setAttribute("aria-label", "Object transform");
	root.style.display = "none";
	const title = node(document, "title");
	title.textContent = "Transform selected geometry";
	root.append(title);
	canvas.after(root);
	return root;
};

const createControls = (root: SVGSVGElement): Map<string, Control> => {
	const controls = new Map<string, Control>();
	for (const mode of ["rotate", "move"] as const) {
		for (const axis of AXIS_IDS) {
			const handle: Handle = { mode, axis };
			const control = createControl(root.ownerDocument, handle);
			controls.set(keyOf(handle), control);
			root.append(control.group);
		}
	}
	return controls;
};
const widgetContext = (bindings: WidgetBindings): GizmoContext | undefined => {
	const object = bindings.object();
	if (object === undefined || bindings.blocked()) {
		return;
	}
	const { canvas } = bindings;
	const rect = canvas.getBoundingClientRect();
	const projection = makeProjection(
		bindings.camera(),
		[rect.width, rect.height],
		canvas.width / canvas.height,
	);
	if (projection !== undefined) {
		return { object, projection };
	}
	return;
};

const drawGizmo = (
	root: SVGSVGElement,
	controls: ReadonlyMap<string, Control>,
	current: GizmoContext | undefined,
): void => {
	if (current === undefined) {
		root.style.display = "none";
		return;
	}
	const layout = layoutGizmo(current.projection, current.object.position);
	if (layout === undefined) {
		root.style.display = "none";
		return;
	}
	root.style.display = "block";
	root.setAttribute(
		"viewBox",
		`0 0 ${current.projection.width} ${current.projection.height}`,
	);
	for (const geometry of layout.handles) {
		const control = controls.get(keyOf(geometry.handle));
		if (control !== undefined) {
			drawControl(control, geometry);
		}
	}
};

// Projected world-space handles; no animation loop. Only changed scene/view state redraws.
const createGizmo = (bindings: WidgetBindings): GizmoWidget => {
	const { canvas } = bindings;
	const document = canvas.ownerDocument;
	const keyboard = document.defaultView;
	if (keyboard === null) {
		throw new Error("Gizmo needs an attached canvas");
	}
	const root = createRoot(canvas);
	const controls = createControls(root);
	const context = (): GizmoContext | undefined => widgetContext(bindings);
	const input = listenGizmo({
		surface: root,
		keyboard,
		visibility: document,
		context,
		hidden: () => document.hidden,
		blocked: bindings.blocked,
		handle: (target: EventTarget | null): Handle | undefined => {
			const group = controlAt(root, target);
			if (group === null) {
				return;
			}
			return controls.get(
				String(group.dataset.gizmoMode) + String(group.dataset.gizmoAxis),
			)?.handle;
		},
		start: (handle: Handle): void => {
			controls.get(keyOf(handle))?.group.focus();
			bindings.start();
		},
		change: bindings.change,
		error: bindings.error,
		active: (handle: Handle | undefined): void => {
			for (const control of controls.values()) {
				control.group.classList.toggle(
					"dragging",
					control.handle.axis === handle?.axis &&
						control.handle.mode === handle?.mode,
				);
			}
		},
	});
	const update = (): void => {
		input.sync();
		drawGizmo(root, controls, context());
	};
	return {
		update,
		dragging: input.dragging,
		cancel: input.cancel,
		dispose: (): void => {
			input.dispose();
			root.remove();
		},
	};
};
export { createGizmo, type GizmoWidget };
