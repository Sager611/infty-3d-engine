// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Vanilla DOM adapter with an isolated pure mapping.
import { MAX_SPEED, MIN_SPEED, travelSpeed } from "./camera.ts";

const READOUT_DIGITS = 3;
const POSITION_ERROR = 1e-15;
type VelocityNode = EventTarget &
	Pick<
		HTMLInputElement,
		"value" | "valueAsNumber" | "ariaValueText" | "dataset"
	>;
type VelocityBindings = Readonly<{
	boost: () => boolean;
	set: (speed: number) => void;
	error: (error: unknown) => void;
}>;
const maximum = (boost?: boolean): number =>
	MAX_SPEED * (1 + Number(boost === true));
function speedAt(position: number, boost?: boolean): number {
	if (!Number.isFinite(position) || position < 0 || position > 1) {
		throw new Error("Invalid speed position");
	}
	if (position === 0) {
		return MIN_SPEED;
	}
	if (position === 1) {
		return maximum(boost);
	}
	return MIN_SPEED * Math.exp(position * Math.log(maximum(boost) / MIN_SPEED));
}
function speedPosition(speed: number, boost?: boolean): number {
	if (!Number.isFinite(speed) || speed < MIN_SPEED || speed > maximum(boost)) {
		throw new Error("Invalid travel speed");
	}
	return Math.log(speed / MIN_SPEED) / Math.log(maximum(boost) / MIN_SPEED);
}
function renderVelocity(
	node: VelocityNode,
	speed: number,
	boost?: boolean,
): void {
	const shown = travelSpeed(speed, 0) * (1 + Number(boost === true));
	const position = speedPosition(shown, boost);
	const rate = String(shown);
	const boosted = String(boost === true);
	if (
		Math.abs(node.valueAsNumber - position) <= POSITION_ERROR &&
		node.dataset.speed === rate &&
		node.dataset.boosted === boosted
	) {
		return;
	}
	node.value = String(position);
	node.dataset.speed = rate;
	node.dataset.boosted = boosted;
	node.ariaValueText = `${shown.toPrecision(READOUT_DIGITS)} units per second`;
}
function listenVelocity(
	node: VelocityNode,
	bindings: VelocityBindings,
): () => void {
	function input(): void {
		try {
			const boost = bindings.boost();
			bindings.set(
				Math.max(
					MIN_SPEED,
					speedAt(node.valueAsNumber, boost) / (1 + Number(boost)),
				),
			);
		} catch (error) {
			bindings.error(error);
		}
	}
	node.addEventListener("input", input);
	return () => node.removeEventListener("input", input);
}
export { speedAt, speedPosition, renderVelocity, listenVelocity };
