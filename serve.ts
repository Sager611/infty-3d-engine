// biome-ignore-all lint/nursery/useExplicitType: Bun.serve infers strict TypeScript types without redundant annotations.
// biome-ignore-all lint/nursery/useQwikValidLexicalScope: This vanilla Bun server does not use Qwik resumability.

const headers = {
	"Cross-Origin-Opener-Policy": "same-origin",
	"Cross-Origin-Embedder-Policy": "require-corp",
};

const fileFor = (path: string): [string, string] | undefined => {
	switch (path) {
		case "/":
		case "/index.html":
			return ["index.html", "text/html; charset=utf-8"];
		case "/dist/app.js":
			return ["dist/app.js", "text/javascript; charset=utf-8"];
		case "/pkg/infty.js":
			return ["pkg/infty.js", "text/javascript; charset=utf-8"];
		case "/pkg/infty_bg.wasm":
			return ["pkg/infty_bg.wasm", "application/wasm"];
		default:
			return;
	}
};

Bun.serve({
	hostname: "127.0.0.1",
	port: 4173,
	fetch(request) {
		let path: string;
		try {
			path = new URL(request.url).pathname;
		} catch {
			return new Response("Bad request", { status: 400, headers });
		}
		if (path === "/favicon.ico") {
			return new Response(null, { status: 204, headers });
		}
		const file = fileFor(path);
		if (file === undefined) {
			return new Response("Not found", { status: 404, headers });
		}
		return new Response(Bun.file(file[0]), {
			headers: { ...headers, "Content-Type": file[1] },
		});
	},
});
