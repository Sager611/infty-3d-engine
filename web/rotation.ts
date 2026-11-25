// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Pure math, not Qwik components.
type Vector = readonly [number, number, number];
type Rotation = readonly [number, number, number, number];
type Axis = 0 | 1 | 2;
const IDENTITY_ROTATION: Rotation = Object.freeze([0, 0, 0, 1]);
const NORMALIZATION_ULPS = 4;
const validRotation = (value: Rotation): boolean =>
	Array.isArray(value) &&
	value.length === IDENTITY_ROTATION.length &&
	value.every(Number.isFinite) &&
	value.some((component) => component !== 0);

function normalizeRotation(value: Rotation): Rotation {
	if (!validRotation(value)) {
		throw new Error("Invalid rotation");
	}
	let result: Rotation = value;
	if (
		Math.abs(Math.hypot(...value) - 1) >
		NORMALIZATION_ULPS * Number.EPSILON
	) {
		const scale = Math.max(...value.map(Math.abs));
		const scaled = [
			value[0] / scale,
			value[1] / scale,
			value[2] / scale,
			value[3] / scale,
		] as const;
		const length = Math.hypot(...scaled);
		result = [
			scaled[0] / length,
			scaled[1] / length,
			scaled[2] / length,
			scaled[3] / length,
		];
	}
	const first =
		[result[3], result[0], result[1], result[2]].find((v) => v !== 0) ?? 1;
	const clean = (v: number): number => {
		if (v === 0) {
			return 0;
		}
		return v * Math.sign(first);
	};
	return Object.freeze([
		clean(result[0]),
		clean(result[1]),
		clean(result[2]),
		clean(result[3]),
	]);
}

function rotateVector(rotation: Rotation, point: Vector): Vector {
	const [x, y, z, w] = normalizeRotation(rotation);
	const [px, py, pz] = point;
	const ux = y * pz - z * py;
	const uy = z * px - x * pz;
	const uz = x * py - y * px;
	const scale = 2 / (x * x + y * y + z * z + w * w);
	return [
		px + scale * (w * ux + y * uz - z * uy),
		py + scale * (w * uy + z * ux - x * uz),
		pz + scale * (w * uz + x * uy - y * ux),
	];
}

function turnRotation(
	rotation: Rotation | undefined,
	axis: Axis,
	angle: number,
): Rotation {
	if (!Number.isFinite(angle)) {
		throw new Error("Invalid rotation angle");
	}
	const [x, y, z, w] = normalizeRotation(rotation ?? IDENTITY_ROTATION);
	const turn: [number, number, number, number] = [0, 0, 0, Math.cos(angle / 2)];
	turn[axis] = Math.sin(angle / 2);
	const [tx, ty, tz, tw] = turn;
	return normalizeRotation([
		tw * x + tx * w + ty * z - tz * y,
		tw * y - tx * z + ty * w + tz * x,
		tw * z + tx * y - ty * x + tz * w,
		tw * w - tx * x - ty * y - tz * z,
	]);
}

export {
	IDENTITY_ROTATION,
	normalizeRotation,
	rotateVector,
	turnRotation,
	validRotation,
	type Axis,
	type Rotation,
	type Vector,
};
