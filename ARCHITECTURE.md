# Architecture

Pure values describe the scene; thin adapters own browser and GPU effects.

| Module | Contract |
|---|---|
| `src/math/` | f64 rays, roots, UVs, normals, footprints and gap estimates; typed numerical errors |
| `src/geometry.rs`, `src/geometry/` | Validated canonical definitions, kernel dispatch and bounded closest-query contracts |
| `src/scene/` | Immutable shared definitions/instances, uniform transforms, reusable hierarchy and deterministic queries |
| `src/api.rs` | WASM packet adapters; invalid inputs become JavaScript errors |
| `src/diagnostic.rs` | Caller-owned, opt-in bounded numerical records; no normal-path trace allocation or logging |
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

Closest-point request3 is `[absolute,relative,maxEvaluations]`; maximum65,536 evaluations, default4,096. Closest12 is `[resolution,multiplicity,hasWitness,lower,upper,x,y,z,positionError,evaluations,0,0]`. Resolution0/1 is converged/unresolved; convergence certifies unsigned distance width <= absolute+relative*upper, not coordinate uniqueness. Multiplicity0/1/2 is proven unique/non-unique/unknown. Position error encloses a surface point near the reported witness, not its separation from an exact global minimizer. Zero work returns no witness and `[0,infinity]`, never a miss.

Sphere/plane closest queries are analytic. Quartic closest search reflects into the positive octant and covers the surface with six analytic two-coordinate charts; maximum-gradient-axis coverage retains ties. Outward-rounded box and separable Lagrangian Taylor bounds certify global distance, while root-enclosed surface witnesses supply upper bounds. Refinement compares all remaining patches, not one projection/Newton start. One evaluation is a bounded seed/patch computation; heap storage is bounded by work. Exhausted work or precision returns unresolved bounds. Exact symmetries and strictly convex lifted exterior problems prove multiplicity; other cases remain unknown.

`closest_point(kind,parameter,point3,request3)` queries canonical geometry. `PreparedScene.closest_point(id,worldPoint3,request3)` queries one instance by stable ID, not the whole scene. Uniform transforms convert absolute accuracy, distances and errors once; query-coordinate uncertainty widens distance bounds and may invalidate multiplicity. World witness rounding remains separate. Nonuniform transforms are not representable by this interface. Invalid scalar kinds/modes/IDs are rejected before WASM integer coercion; overflow/underflow in required coordinate conversion is explicit.

## Bounded diagnostics

Normal queries use disabled recorders, with no trace storage or logs. Explicit `trace_closest_point(kind,parameter,point3,request3,maxEvents)`, `PreparedScene.trace_closest_point(id,worldPoint3,request3,maxEvents)` and `PreparedScene.trace_ray(ray6,mode,maxEvents)` preserve the ordinary result and work budget. Limits are integral0..256, validated before integer coercion. A full log retains its prefix and counts omitted events (saturating u32); it does not stop or alter solving. Errors remain errors.

Trace packets contain header3 `[resultValueCount,eventCount,omittedCount]`, the unmodified closest12/query24N result, then rows8 `[tag,objectId,a,b,c,d,e,f]`. Object0 denotes canonical queries or scene-wide traversal. Root coordinates are ray parameters relative to the enclosure's shift; chart bounds are canonical (reflected positive octant). Only world-tagged rows have instance distance/error units. The final result is authoritative: a truncated trace may omit its final bounds.

| Tag | Six values |
|---|---|
| 1 enclosure | lo, hi, ray shift, search radius, 0, 0 |
| 2 root candidate | coordinate, polynomial value, evaluation allowance, 0, 0, 0; not proof of an exact repeated root |
| 3 sign bracket | lo, hi, refined coordinate, bracket error, endpoint values |
| 4 rejection | reason1: interval lo/hi, endpoint values, sign product; reason2: empty lo/hi, shift, 0, 0; reason4: chart axis/outer, inherited lower, best upper, evaluation |
| 5/6 canonical/world ray | hit, t, lower gap, upper gap, numerical error, status |
| 7/8 canonical/world closest | lower, upper, witness position error, evaluations, resolution, multiplicity |
| 9 chart | axis, outer-root flag, lower, current best upper, evaluation, retained flag |
| 10 traversal | instance count, candidate count, query mode, 0, 0, 0 |

Ray status2 explicitly marks uncomputed gap/witness fields; first/all discard those misses. They must not be interpreted as valid uppers. Ordinary public query24 remains status0/1 only. Explicit state replaces an overflowing numerical-error sentinel for large scaled misses. Polynomial overflow and underflowed nonzero discriminants report numerical errors, not fabricated roots.

## State and effects

Right controls are a native disclosure, closed by default. Plain clicks pick first; Alt/pick-all uses the clicked ray's hits. Drags/cancel/multitouch do not become picks. Selection is transient; one active object supplies metadata-driven controls.

Add chooses bounded candidates in the current camera plane, preferring separation while keeping finite enclosures in view; overlap is possible in crowded scenes. Portrait aspect reduces defaults. Capacity or an impossibly close view is an explicit error. IDs wrap past u32::MAX and skip live instances. Delete removes only selected instances. Undo retains one pre-deletion scene/selection (at most65,536 objects); another deletion replaces it. Add, property edits, reset and external URL loads invalidate it; camera, display and selection changes do not. Undo restores scene/selection, never the camera. Escape/light-dismiss closes the native shape picker; Delete/Backspace ignore editable controls, open picker, modifiers and repeats.

Saved scene URLs use `#s=1;camera6;mode;objects8N`. Legacy `#v=yaw,pitch,distance,radius,mode` keeps the original centered sphere/plane scene. URL replacement is single-shot debounced150ms. Updates are absolute/idempotent and copy caller-owned coordinates.

WASD moves relative to the view; drag/arrows look from a fixed eye, including during translation. Wheel/+/- adjust speed[0.01,100] units/s(default3), never zoom. Diagonals retain base speed; steps cap at50ms. Blur, hidden pages, editable focus, menus, reset/framing, external loads and disposal clear held keys. Motion alone schedules successor frames; idle scenes have no RAF. Speed is transient; poses retain Camera6 saved compatibility.

Only sole W queries the nearest analytic center-ray hit. Inside one world unit, speed eases linearly with remaining distance down to0.01 units/s. Closed-form integration handles band entry/contact within a capped step, then continues through at the floor; this is not collision prevention. Reverse/strafe/diagonal/cancelling chords retain base speed and skip the query. Unresolved hits/errors stop motion explicitly, never become misses.

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
| 2026-11-09 | Assistant | Added W-only analytic approach easing with closed-form steps | Inspect nearby surfaces without slowing escape or implying collision prevention |
| 2026-11-09 | Assistant | Added globally bounded quartic closest queries, uniform adaptation and strict scalar validation | Separate global accuracy, surface error, ties and exhausted work without silent coercion |
| 2026-11-09 | Assistant | Added bounded opt-in numerical traces and independent edge-case oracles | Explain numerical decisions without idle work, false roots or scaled-miss overflow |
