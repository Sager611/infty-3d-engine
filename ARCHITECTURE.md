# Architecture

Pure values describe the scene; thin adapters own browser and GPU effects.

| Module | Contract |
|---|---|
| `src/math/` | f64 rays, roots, UVs, normals, footprints and gap estimates; typed numerical errors |
| `src/geometry.rs`, `src/geometry/` | Validated canonical definitions, kernel dispatch and bounded closest-query contracts |
| `src/scene/` | Immutable shared definitions/instances, uniform transforms, reusable hierarchy and deterministic queries |
| `src/api.rs` | WASM packet adapters; invalid inputs become JavaScript errors |
| `src/{gpu,upload}.rs` | Explicit render inputs, derived uploads, buffers, resize and acquisition; no cached scene/camera model |
| `src/shaders/` | f32 mirror of geometry/material contracts; shared compile-time shader manifest |
| `web/{scene,state,query,camera}.ts` | Immutable records, absolute updates, URL codec, query decoding and framing |
| `web/editing.ts` | Pure add/delete, stable IDs, selection repair and one-deletion undo |
| `web/scene-actions.ts` | Delegated scene buttons and guarded delete keys; explicit cleanup |
| `web/{controls,input,keys,frames,metrics}.ts` | Small DOM/input/timer adapters with cleanup |
| `web/status.ts` | Immutable action/render messages; redraws clear only render failures, and empty messages share canonical state |
| `web/prepared.ts` | Prepared scene ownership and safe deferred GPU acquisition |
| `web/app.ts` | Snapshot, selection, queries, invalidation and resource lifecycle |

## Data boundaries

Object stride8: `[id,kind,px,py,pz,size,parameter,0]`. Kinds1/2/3 are sphere/Goursat/plane. Empty scenes are valid; maximum65,536 objects, unique IDs1..u32::MAX, position components[-20,20], size[0.01,4], Goursat beta[1.05,1.95]. Other parameters/padding are zero. Reject invalid packets; do not silently clamp them.

Camera6: `[yaw,pitch,distance,targetX,targetY,targetZ]`. Normalize finite yaw; constrain pitch[-1.45,1.45], distance[0.0001,100], target[-1024,1024]. Camera travel is independent of the scene's[-20,20] editing bounds; the finite camera limit remains explicit. Query screen4 is `[width,height,x,y]`; browser picks floor/clamp CSS coordinates before pixel-center ray construction.

`query_scene(objects8N,camera6,screen4,mode)` and `query_ray(objects8N,ray6,mode)` use mode0 first hit,1 all hits,2 inspection including misses. One nearest forward hit per object; hit order is(world t,id). Normalize ray direction. Uniform transforms use `world=position+size*local`, scaling distances/errors once.

Query24: `[id,kind,hit,t,gapLower,gapUpper,px,py,pz,nx,ny,nz,u,v,rayX,rayY,rayZ,localX,localY,localZ,error,gapKind,status,0]`. Gap kind0 is analytic,1 bounded; public status0 is okay,1 uncertain. Hit gaps are zero. Missing witnesses are errors, not fabricated finite results.

`Renderer.render_scene(camera6,preparedScene,displayMode,selectedIds)` receives the immutable model explicitly; legacy `render(camera6,objects8N,displayMode,selectedIds)` prepares a temporary equivalent. GPU frame/record layouts are96/48 bytes; scene storage is binding5. The buffer grows geometrically, only when needed. A deterministic median-split, stackless threaded hierarchy encloses finite objects; planes remain unbounded leaves. Each instance stores its own ID/transform/selection flag and references an interned (kind, parameter) geometry record. No selection bitmask or fixed shader object loop remains. CPU hit queries use the same hierarchy; inspection intentionally visits every instance. Parsing and selection use ordered sets rather than quadratic scans. Initialization validates the pipeline; surface acquisition failures are explicit. Device recreation requires reloading rather than a background retry loop.

`PreparedScene(objects8N)` owns immutable definition/instance arrays, stable-ID lookup and hierarchy. Its camera/ray queries reuse them; legacy packet functions prepare a temporary equivalent. Instances contain a definition index, positive uniform transform and material reference (default0 only). Editing creates a replacement scene and never mutates shared peers. Browser rendering, picks and inspection reuse this model. Camera/display frames and equivalent selection sets do not repack or upload geometry. Renderer keeps only a weak scene identity and normalized selection for derived uploads, committed after validation and queuing the write. Scene replacement prepares successfully before releasing the previous WASM resource; disposal releases owned resources, including GPU initialization completing after cleanup. Bounds, rays and samples use the same uniform-transform module.

## Geometry and display

Extension seam: `geometry.rs` supplies validated canonical definitions, analytic queries and an optional enclosing radius. Add a kind/tag, validated parameters, its CPU kernel and matching WGSL dispatch, then web metadata/material mapping. Transforms, hierarchy, shared-geometry interning and selection stay unchanged. This is a compiled analytic-kernel interface, not runtime shader loading; the existing sphere, quartic and unbounded plane exercise it without a new shape catalog.

Primary GPU traversal expands branch bounds for the maximum possible pixel outline cone and the visible gap halo; ray-only rejection is reserved for shadows and CPU hit queries. Thus misses just outside an object's ray bounds remain candidates. Slab tests explicitly handle parallel rays and inflate for rounding. Overlapping geometry or many infinite planes can degrade to exhaustive traversal; no acceleration changes exact first/all ordering or inspection cardinality.


Sphere/two-sided-plane queries are analytic. The Goursat surface is `sum((p*p-1)^2)-beta=0`, enclosed by radius `sqrt(3+sqrt(3*beta))`. Cubic stationary roots isolate quartic intervals; finite refinement is not spatial ray marching.

Sphere/plane gaps are analytic within floating-point precision. Quartic misses have an enclosing-sphere lower bound and a bracketed surface-witness upper bound, including position/rounding allowance. Numerical error is separate from interval width: never add it to the upper bound again. Bounds can be loose; uncertain results are labeled, not called exact closest distances.

Selected sphere/quartic misses qualify for an outline only when their valid upper gap is below the pixel-scale thickness and their witness passes the depth test. No duplicate enlarged surfaces or hit fills. Loose witnesses can under-cover; planes have no finite silhouette. Internal shader status2/3 means uncomputed/invalid and cannot qualify.

Modes0/1/2 are UV/Gap/Detail. Canonical UVs and analytic footprints apply object size once. Detail blends three adjacent hashed power-of-two grids with finite precision/LOD limits, not texture images or literally infinite detail.

Closest-point request3 is `[absolute,relative,maxEvaluations]`; maximum65,536 evaluations, default4,096. Closest12 is `[resolution,multiplicity,hasWitness,lower,upper,x,y,z,positionError,evaluations,0,0]`. Resolution0/1 is converged/unresolved; multiplicity0/1/2 is unique/non-unique/unknown. Zero work returns no witness and `[0,infinity]`, never a miss. Distances are unsigned and position error is separate. Sphere/plane adapters are analytic; quartic closest queries remain explicitly unsupported until the bounded solver is integrated.

## State and effects

Right controls are a native disclosure, closed by default. Plain clicks pick first; Alt/pick-all uses the clicked ray's hits. Drags/cancel/multitouch do not become picks. Selection is transient; one active object supplies metadata-driven controls.

Add chooses bounded candidates in the current camera plane, preferring separation while keeping finite enclosures in view; overlap is possible in crowded scenes. Portrait aspect reduces defaults. Capacity or an impossibly close view is an explicit error. IDs wrap past u32::MAX and skip live instances. Delete removes only selected instances. Undo retains one pre-deletion scene/selection (at most65,536 objects); another deletion replaces it. Add, property edits, reset and external URL loads invalidate it; camera, display and selection changes do not. Undo restores scene/selection, never the camera. Escape/light-dismiss closes the native shape picker; Delete/Backspace ignore editable controls, open picker, modifiers and repeats.

Saved scene URLs use `#s=1;camera6;mode;objects8N`. Legacy `#v=yaw,pitch,distance,radius,mode` keeps the original centered sphere/plane scene. URL replacement is single-shot debounced150ms. Updates are absolute/idempotent and copy caller-owned coordinates.

WASD moves relative to the view; drag/arrows look from a fixed eye, including during translation. Wheel/+/- adjust speed[0.01,100] units/s(default3), never zoom. Diagonals retain base speed; steps cap at50ms. Blur, hidden pages, editable focus, menus, reset/framing, external loads and disposal clear held keys. Motion alone schedules successor frames; idle scenes have no RAF. Speed is transient; poses retain Camera6 saved compatibility. W-only approach easing remains pending.

Render after changes or active motion, never idle. Visible-only1Hz metrics report CPU submission walltime and optional JS heap/WASM bytes—not CPU utilization, GPU time or total browser/GPU memory. Input/timer resources have explicit disposal.

Run `./check`: strict native/WASM compilation and lint, Rust/Naga/real-GPU tests, TypeScript/Biome, Bun tests, shell/Markdown and dependency checks. Browser screenshots/interaction checks remain separate acceptance evidence.

## Changelog
| When | Who | What | Why |
|---|---|---|---|
| 2026-11-08 | Assistant | Documented modules, packets, geometry, state and limits | Make behavior and effect ownership explicit |
| 2026-11-08 | Assistant | Added shared geometry, scalable selection and threaded finite bounds | Support thousands without dropping gap/outline candidates |
| 2026-11-08 | Assistant | Documented instance editing and bounded deletion undo | Make invalidation, allocation and keyboard behavior explicit |
| 2026-11-08 | Assistant | Split canonical query contracts from scene effects | Centralize validation, accuracy and unresolved outcomes |
| 2026-11-08 | Assistant | Shared immutable definitions/instances across CPU and GPU adapters | Prepare once, preserve peers and centralize uniform transforms |
| 2026-11-08 | Assistant | Reused prepared scenes through browser/GPU ownership | Avoid camera-frame rebuilds/uploads and late initialization leaks |
| 2026-11-08 | Assistant | Added tested camera navigation math and expanded camera-only bounds | Prepare free movement without changing scene editing limits |
| 2026-11-09 | Assistant | Wired guarded free-camera controls, motion frames and persistent action errors | Replace orbit/zoom without idle rendering or silent input failures |
