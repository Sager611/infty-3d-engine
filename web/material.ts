// biome-ignore-all lint/nursery/useQwikValidLexicalScope: Pure catalogue.
type MaterialKind = "steel" | "brick" | "leaf";
type MaterialDescriptor = Readonly<{ tag: number; label: string }>;
const MATERIALS: Readonly<Record<MaterialKind, MaterialDescriptor>> =
	Object.freeze({
		steel: Object.freeze({ tag: 0, label: "Brushed steel" }),
		brick: Object.freeze({ tag: 1, label: "Red brickwork" }),
		leaf: Object.freeze({ tag: 2, label: "Leaf" }),
	});
const isMaterial = (value: unknown): value is MaterialKind =>
	typeof value === "string" && Object.hasOwn(MATERIALS, value);
const materialKind = (tag: number): MaterialKind | undefined => {
	if (tag === MATERIALS.steel.tag) {
		return "steel";
	}
	if (tag === MATERIALS.brick.tag) {
		return "brick";
	}
	if (tag === MATERIALS.leaf.tag) {
		return "leaf";
	}
	return;
};
export { MATERIALS, isMaterial, materialKind, type MaterialKind };
