# infty

Analytic Rust + WebGPU ray queries: instanced geometry, closest points and heightmap relief, no ray marching.

**Live demo:** <https://sager611.github.io/infty-3d-engine/> (needs a WebGPU browser). WASD moves, drag looks, wheel sets speed, F frames, R resets.

## Develop

```sh
bun install
bun run build:wasm && bun run build:web
bun run dev   # http://127.0.0.1:4173
./check       # format, lint, tests
```

## Credits

Texture and relief sources, licenses and changes: [materials](src/material/REFERENCES.md), [leaf](src/heightfield/REFERENCES.md).

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-27 | Assistant | Created README with demo link and credits | Public repo and demo |
