type FrameDriver = Readonly<{
	request: (tick: (now: number) => void) => number;
	cancel: (id: number) => void;
	visible: () => boolean;
	moving: () => boolean;
	draw: (now: number) => void;
}>;
type Frames = Readonly<{
	invalidate(): void;
	visibility(): void;
	dispose(): void;
}>;
// Owns one pending callback. Draw errors propagate and do not schedule a retry.
function createFrames(driver: FrameDriver): Frames {
	let pending: number | undefined;
	let disposed = false;
	function tick(now: number): void {
		pending = undefined;
		if (disposed || !driver.visible()) {
			return;
		}
		driver.draw(now);
		if (driver.moving()) {
			invalidate();
		}
	}
	function invalidate(): void {
		if (!disposed && pending === undefined && driver.visible()) {
			pending = driver.request(tick);
		}
	}
	function cancel(): void {
		if (pending !== undefined) {
			driver.cancel(pending);
			pending = undefined;
		}
	}
	function visibility(): void {
		if (driver.visible()) {
			invalidate();
		} else {
			cancel();
		}
	}
	function dispose(): void {
		disposed = true;
		cancel();
	}
	return Object.freeze({ invalidate, visibility, dispose });
}
// Resolves on the frame after the next: a frame starts only once the previous one is shown.
function presented(request: FrameDriver["request"]): Promise<void> {
	return new Promise((resolve) => {
		request(() => {
			request(() => {
				resolve();
			});
		});
	});
}
export { createFrames, type FrameDriver, type Frames, presented };
