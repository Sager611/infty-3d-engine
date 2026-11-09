// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
// biome-ignore-all lint/style/noMagicNumbers: Packet indices and locked protocol flags are contract values.
import { type SceneKind, tagKind } from "./scene.ts";

type Triple = readonly [number, number, number];
type Pair = readonly [number, number];

const canvasPixel = (
	size: Pair,
	rect: Readonly<{ left: number; top: number; width: number; height: number }>,
	point: Pair,
): Pair => {
	const [width, height] = size;
	if (
		!(
			size.every((n) => Number.isSafeInteger(n) && n > 0) &&
			[...size, ...point, rect.left, rect.top, rect.width, rect.height].every(
				Number.isFinite,
			)
		) ||
		rect.width <= 0 ||
		rect.height <= 0
	) {
		throw new Error("canvas has no drawable area");
	}
	return Object.freeze([
		Math.min(
			width - 1,
			Math.max(0, Math.floor(((point[0] - rect.left) / rect.width) * width)),
		),
		Math.min(
			height - 1,
			Math.max(0, Math.floor(((point[1] - rect.top) / rect.height) * height)),
		),
	]);
};

type QueryResult = Readonly<{
	id: number;
	kind: SceneKind;
	hit: boolean;
	t: number;
	gapLower: number;
	gapUpper: number;
	point: Triple;
	normal: Triple;
	uv: Pair;
	rayPoint: Triple;
	local: Triple;
	error: number;
	bounds: boolean;
	uncertain: boolean;
}>;

type Selection = Readonly<{
	ids: readonly number[];
	active: number | null;
}>;

const QUERY_STRIDE = 24;
const MAX_ID = 0xff_ff_ff_ff;
const EMPTY_IDS: readonly number[] = Object.freeze([]);
const EMPTY_SELECTION: Selection = Object.freeze({
	ids: EMPTY_IDS,
	active: null,
});

const invalid = (message: string): never => {
	throw new Error(message);
};

const valueAt = (packet: ArrayLike<number>, index: number): number => {
	const value = packet[index];
	if (value === undefined) {
		invalid("nonfinite query value");
	}
	if (!Number.isFinite(value)) {
		invalid("nonfinite query value");
	}
	return Number(value);
};

const kindAt = (tag: number): SceneKind =>
	tagKind(tag) ?? invalid("invalid kind");

const flagAt = (value: number, name: string): boolean => {
	if (value !== 0 && value !== 1) {
		invalid(`invalid ${name} flag`);
	}
	return value === 1;
};

const nonnegative = (value: number, name: string): number => {
	if (value < 0) {
		invalid(`invalid ${name} distance`);
	}
	return value;
};

const triple = (x: number, y: number, z: number): Triple =>
	Object.freeze([x, y, z]);

const pair = (x: number, y: number): Pair => Object.freeze([x, y]);

type QueryMetadata = Readonly<{
	id: number;
	kind: SceneKind;
	hit: boolean;
	t: number;
	gapLower: number;
	gapUpper: number;
	error: number;
	bounds: boolean;
	uncertain: boolean;
}>;

const queryMetadata = (
	packet: ArrayLike<number>,
	offset: number,
): QueryMetadata => {
	const id = valueAt(packet, offset);
	if (!Number.isInteger(id) || id < 1 || id > MAX_ID) {
		invalid("invalid id");
	}
	const kind = kindAt(valueAt(packet, offset + 1));
	const hit = flagAt(valueAt(packet, offset + 2), "hit");
	const t = nonnegative(valueAt(packet, offset + 3), "t");
	const gapLower = nonnegative(valueAt(packet, offset + 4), "gap lower");
	const gapUpper = nonnegative(valueAt(packet, offset + 5), "gap upper");
	if (gapLower > gapUpper) {
		invalid("invalid gap bounds");
	}
	const error = nonnegative(valueAt(packet, offset + 20), "error");
	const bounds = flagAt(valueAt(packet, offset + 21), "bounds");
	const uncertain = flagAt(valueAt(packet, offset + 22), "status");
	if (error === Number.MAX_VALUE) {
		invalid("uncomputed query");
	}
	if (valueAt(packet, offset + 23) !== 0) {
		invalid("invalid padding");
	}
	if (hit && (bounds || gapLower !== 0 || gapUpper !== 0)) {
		invalid("hit gap must be zero");
	}
	return { id, kind, hit, t, gapLower, gapUpper, error, bounds, uncertain };
};

const decodeQuery = (
	packet: ArrayLike<number>,
	offset: number,
): QueryResult => {
	const metadata = queryMetadata(packet, offset);
	return Object.freeze({
		...metadata,
		point: triple(
			valueAt(packet, offset + 6),
			valueAt(packet, offset + 7),
			valueAt(packet, offset + 8),
		),
		normal: triple(
			valueAt(packet, offset + 9),
			valueAt(packet, offset + 10),
			valueAt(packet, offset + 11),
		),
		uv: pair(valueAt(packet, offset + 12), valueAt(packet, offset + 13)),
		rayPoint: triple(
			valueAt(packet, offset + 14),
			valueAt(packet, offset + 15),
			valueAt(packet, offset + 16),
		),
		local: triple(
			valueAt(packet, offset + 17),
			valueAt(packet, offset + 18),
			valueAt(packet, offset + 19),
		),
	});
};

const decodeQueries = (packet: ArrayLike<number>): readonly QueryResult[] => {
	if (
		!Number.isInteger(packet.length) ||
		packet.length < 0 ||
		packet.length % QUERY_STRIDE !== 0
	) {
		invalid("invalid query length");
	}
	const results: QueryResult[] = [];
	for (let offset = 0; offset < packet.length; offset += QUERY_STRIDE) {
		results.push(decodeQuery(packet, offset));
	}
	return Object.freeze(results);
};

const selectionFromHits = (results: readonly QueryResult[]): Selection => {
	const ids: number[] = [];
	const seen = new Set<number>();
	for (const result of results) {
		if (result.hit && !seen.has(result.id)) {
			seen.add(result.id);
			ids.push(result.id);
		}
	}
	if (ids.length === 0) {
		return EMPTY_SELECTION;
	}
	return Object.freeze({
		ids: Object.freeze(ids),
		active: ids[0] ?? null,
	});
};

const activateSelection = (selection: Selection, id: number): Selection => {
	if (!selection.ids.includes(id)) {
		invalid("id is not selected");
	}
	if (selection.active === id) {
		return selection;
	}
	return Object.freeze({ ids: selection.ids, active: id });
};

const vectorText = (value: readonly number[]): string =>
	value.map((n) => n.toFixed(3)).join(", ");

const inspectResult = (result: QueryResult): string => {
	let qualifier = "miss";
	if (result.hit) {
		qualifier = "hit";
	}
	if (result.uncertain) {
		qualifier = `uncertain ${qualifier}`;
	}
	if (result.bounds) {
		qualifier += " (bounded)";
	}
	const lower = (Math.floor(result.gapLower * 1000) / 1000).toFixed(3);
	const upper = (Math.ceil(result.gapUpper * 1000) / 1000).toFixed(3);
	let distance = `gap [${lower}, ${upper}]`;
	if (result.hit) {
		distance = `t ${result.t.toFixed(3)}`;
	}
	return `#${result.id} ${result.kind}: ${qualifier} · ${distance}
UV (${vectorText(result.uv)}) · normal (${vectorText(result.normal)})
surface (${vectorText(result.point)}) · ray (${vectorText(result.rayPoint)})
error ≈ ${result.error.toExponential(2)}`;
};

export {
	inspectResult,
	EMPTY_SELECTION,
	canvasPixel,
	activateSelection,
	decodeQueries,
	selectionFromHits,
	type Pair,
	type QueryResult,
	type Selection,
	type Triple,
};
