// Bound shader-compile load: compile the scene pipeline off the browser's GPU thread, one compile at a time
// across tabs; the renderer's identical pipeline then comes from the browser's shader cache.
type CompileDevice = Readonly<{
	createShaderModule(descriptor: { code: string }): object;
	createComputePipelineAsync(descriptor: {
		layout: "auto";
		compute: { module: object; entryPoint: string };
	}): Promise<object>;
	destroy(): void;
}>;
type CompileGpu = Readonly<{
	requestAdapter(): Promise<{ requestDevice(): Promise<CompileDevice> } | null>;
}>;
type CompileLocks = Readonly<{
	request<T>(name: string, task: () => Promise<T>): Promise<T>;
}>;

// True when the draw pipeline compiled; failures only cost the cache warm-up.
async function prewarm(
	gpu: CompileGpu | undefined,
	code: string,
): Promise<boolean> {
	const device = await (await gpu?.requestAdapter())?.requestDevice();
	if (device === undefined) {
		return false;
	}
	try {
		await device.createComputePipelineAsync({
			layout: "auto",
			compute: {
				module: device.createShaderModule({ code }),
				entryPoint: "draw",
			},
		});
		return true;
	} catch {
		return false;
	} finally {
		device.destroy();
	}
}

function compileExclusively<T>(
	locks: CompileLocks | undefined,
	task: () => Promise<T>,
): Promise<T> {
	if (locks === undefined) {
		return task();
	}
	return locks.request("infty-shader-compile", task);
}

export { type CompileGpu, compileExclusively, prewarm };
