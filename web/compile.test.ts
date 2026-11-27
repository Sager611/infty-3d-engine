// biome-ignore lint/nursery/noUnresolvedImports: Bun provides this virtual module.
import { expect, test } from "bun:test";
import { type CompileGpu, compileExclusively, prewarm } from "./compile.ts";

function fakeGpu(fail: boolean): { calls: string[]; gpu: CompileGpu } {
	const calls: string[] = [];
	const device = {
		createShaderModule(descriptor: { code: string }): object {
			calls.push(`module:${descriptor.code}`);
			return {};
		},
		createComputePipelineAsync(descriptor: {
			layout: "auto";
			compute: { module: object; entryPoint: string };
		}): Promise<object> {
			calls.push(
				`pipeline:${descriptor.layout}:${descriptor.compute.entryPoint}`,
			);
			if (fail) {
				return Promise.reject(new Error("compile failed"));
			}
			return Promise.resolve({});
		},
		destroy(): void {
			calls.push("destroy");
		},
	};
	return {
		calls,
		gpu: {
			requestAdapter: () =>
				Promise.resolve({ requestDevice: () => Promise.resolve(device) }),
		},
	};
}

test("prewarm compiles the draw pipeline asynchronously and always frees its device", async () => {
	const compiled = fakeGpu(false);
	const failed = fakeGpu(true);
	const expected = ["module:wgsl", "pipeline:auto:draw", "destroy"];
	expect(await prewarm(compiled.gpu, "wgsl")).toBe(true);
	expect(await prewarm(failed.gpu, "wgsl")).toBe(false);
	expect(compiled.calls).toEqual(expected);
	expect(failed.calls).toEqual(expected);
	expect(await prewarm(undefined, "wgsl")).toBe(false);
	expect(
		await prewarm({ requestAdapter: () => Promise.resolve(null) }, "wgsl"),
	).toBe(false);
});

test("compiles run one at a time under a shared lock, or directly without locks", async () => {
	const names: string[] = [];
	const locks = {
		request<T>(name: string, task: () => Promise<T>): Promise<T> {
			names.push(name);
			return task();
		},
	};
	expect(await compileExclusively(locks, () => Promise.resolve("locked"))).toBe(
		"locked",
	);
	expect(
		await compileExclusively(undefined, () => Promise.resolve("direct")),
	).toBe("direct");
	expect(names).toEqual(["infty-shader-compile"]);
});
