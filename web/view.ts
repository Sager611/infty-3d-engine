// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This pure module is not a Qwik component.
type ViewMode = 0 | 1 | 2 | 3;

type View = Readonly<{
	yaw: number;
	pitch: number;
	distance: number;
	radius: number;
	mode: ViewMode;
}>;

type RawView = Readonly<Record<keyof View, number | undefined>>;

const INITIAL_YAW = 0.45;
const INITIAL_PITCH = 0.25;
const INITIAL_DISTANCE = 5;
const DEFAULT_RADIUS = 1;
const DEFAULT_MODE: ViewMode = 0;
const MATERIAL_MODE = 3;
const YAW_LIMIT = 100;
const ANGLE_LIMIT = 1.5;
const MAX_DISTANCE = 20;
const MIN_DISTANCE_GAP = 1e-4;
const MIN_RADIUS = 0.25;
const MAX_RADIUS = 1.5;
const MIN_YAW = -YAW_LIMIT;
const MAX_YAW: number = YAW_LIMIT;
const MIN_PITCH = -ANGLE_LIMIT;
const MAX_PITCH: number = ANGLE_LIMIT;
const HASH_PREFIX_LENGTH = 3;
const MAX_FIELDS = 5;

const DEFAULT_VIEW: View = {
	yaw: INITIAL_YAW,
	pitch: INITIAL_PITCH,
	distance: INITIAL_DISTANCE,
	radius: DEFAULT_RADIUS,
	mode: DEFAULT_MODE,
};

const bounded = (
	value: number | undefined,
	fallback: number,
	minimum: number,
	maximum: number,
): number => {
	let finite = fallback;
	if (value !== undefined && Number.isFinite(value)) {
		finite = value;
	}
	return Math.min(maximum, Math.max(minimum, finite));
};

const modeValue = (value: number | undefined): ViewMode => {
	if (value === 1 || value === 2 || value === MATERIAL_MODE) {
		return value;
	}
	return DEFAULT_MODE;
};

const normalized = (raw: RawView): View => {
	const safeRadius = bounded(
		raw.radius,
		DEFAULT_VIEW.radius,
		MIN_RADIUS,
		MAX_RADIUS,
	);
	return {
		yaw: bounded(raw.yaw, DEFAULT_VIEW.yaw, MIN_YAW, MAX_YAW),
		pitch: bounded(raw.pitch, DEFAULT_VIEW.pitch, MIN_PITCH, MAX_PITCH),
		distance: bounded(
			raw.distance,
			DEFAULT_VIEW.distance,
			safeRadius + MIN_DISTANCE_GAP,
			MAX_DISTANCE,
		),
		radius: safeRadius,
		mode: modeValue(raw.mode),
	};
};

const fieldsFromHash = (fields: readonly string[]): RawView => {
	const values = fields.map((field) => {
		if (field.trim() === "") {
			return Number.NaN;
		}
		return Number(field);
	});
	return {
		yaw: values[0],
		pitch: values[1],
		distance: values[2],
		radius: values[3],
		mode: values[4],
	};
};

const parseView = (hash: string): View => {
	if (!hash.startsWith("#v=")) {
		return DEFAULT_VIEW;
	}
	const fields = hash.slice(HASH_PREFIX_LENGTH).split(",");
	if (fields.length > MAX_FIELDS) {
		return DEFAULT_VIEW;
	}
	return normalized(fieldsFromHash(fields));
};

const serializeView = (view: View): string => {
	const safe = normalized(view);
	return `#v=${safe.yaw},${safe.pitch},${safe.distance},${safe.radius},${safe.mode}`;
};

export {
	ANGLE_LIMIT,
	MATERIAL_MODE,
	DEFAULT_MODE,
	DEFAULT_RADIUS,
	DEFAULT_VIEW,
	INITIAL_DISTANCE,
	INITIAL_PITCH,
	INITIAL_YAW,
	MAX_DISTANCE,
	MAX_PITCH,
	MAX_RADIUS,
	MAX_YAW,
	MIN_DISTANCE_GAP,
	MIN_PITCH,
	MIN_RADIUS,
	MIN_YAW,
	parseView,
	serializeView,
	type View,
	type ViewMode,
	YAW_LIMIT,
};
