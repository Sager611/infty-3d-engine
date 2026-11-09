// Pure adapter from canonical scene packets to world-space samples.
struct SceneObject {
  identity: vec4u,
  position_size: vec4f,
  params: vec4f,
};

struct SceneSample {
  query: Query,
  local: vec3f,
  id: u32,
  kind: u32,
  gap_lower: f32,
  error: f32,
  status: u32,
};

fn scene_object(o: vec3f, d: vec3f, obj: SceneObject, need_gap: bool) -> SceneSample {
  let id = obj.identity.x;
  let kind = obj.identity.y;
  let position = obj.position_size.xyz;
  let size = obj.position_size.w;
  let local_o = (o - position) / size;

  var query = Query(false, 0.0, 0.0, local_o,
    vec3f(0.0, 1.0, 0.0), vec2f(0.0));
  var gap_lower = 0.0;
  var error = 0.0;
  var status = 3u;
  if (kind == 1u) {
    query = sphere_at(local_o, d, vec3f(0.0), 1.0);
    gap_lower = query.gap;
    status = 0u;
  } else if (kind == 2u || kind == 4u) {
    var canonical: GeometryQuery;
    if (kind == 2u) { canonical = goursat(local_o, d, obj.params.x, need_gap); }
    else { canonical = crystal(local_o, d, need_gap); }
    query = canonical.query;
    gap_lower = canonical.gap_lower;
    error = canonical.error;
    status = canonical.status;
  } else if (kind == 3u) {
    query = plane(local_o, d);
    gap_lower = query.gap;
    status = 0u;
  }

  let local = query.p;
  query.p = position + size * local;
  query.t = size * query.t;
  query.gap = size * query.gap;
  gap_lower = size * gap_lower;
  error = size * error;
  return SceneSample(query, local, id, kind, gap_lower, error, status);
}
