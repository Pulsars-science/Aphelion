// Shading for the bodies themselves.
//
// Everything here is in *camera-relative render space*: the camera sits at the
// origin and one unit is one astronomical unit. The CPU does the translation in
// f64 before handing over the instance transform, which is what keeps a planet
// smooth when the viewer is thirty AU away. See camera.rs.

struct Globals {
    view_projection: mat4x4<f32>,
    // xyz: the light, camera-relative. w: ambient fraction on the night side.
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct InstanceInput {
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
    // rgb: base colour. a: 1 if the body emits light, 0 if it reflects it.
    @location(7) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );
    let world = model * vec4<f32>(vertex.position, 1.0);

    var out: VertexOutput;
    out.clip_position = globals.view_projection * world;
    out.view_position = world.xyz;
    // The instance transform is a rotation and a uniform scale, so the upper
    // 3x3 carries normals correctly without an inverse-transpose.
    out.normal = normalize((model * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.color = instance.color;
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = in.color.rgb;
    let normal = normalize(in.normal);
    // The camera is at the origin of this space, so the view direction is just
    // the negated position.
    let view_direction = normalize(-in.view_position);

    if (in.color.a > 0.5) {
        // A star. Real limb darkening is a function of optical depth; this is
        // the cheap cosine approximation, which is enough to stop the disc
        // looking like a flat sticker.
        let facing = clamp(dot(normal, view_direction), 0.0, 1.0);
        let limb = 0.55 + 0.45 * pow(facing, 0.45);
        return vec4<f32>(base * limb * 1.7, 1.0);
    }

    let light_direction = normalize(globals.light.xyz - in.view_position);
    let ambient = globals.light.w;

    // Lambert, with the terminator widened slightly. A hard cosine cut-off
    // aliases badly once a planet is only a few pixels across.
    let cosine = dot(normal, light_direction);
    let diffuse = clamp(cosine, 0.0, 1.0);
    let wrapped = clamp((cosine + 0.12) / 1.12, 0.0, 1.0);

    // A weak specular lobe: reads as a curved surface rather than a disc.
    let halfway = normalize(light_direction + view_direction);
    let specular = pow(clamp(dot(normal, halfway), 0.0, 1.0), 24.0) * 0.06 * diffuse;

    // Rim light along the silhouette. This is the cue that sells depth when the
    // body is small on screen, and it costs one dot product.
    let rim = pow(1.0 - clamp(dot(normal, view_direction), 0.0, 1.0), 3.0) * 0.16;

    // Faint latitude banding, strongest at the equator, so that a body's
    // rotation is actually visible without any texture data.
    let banding = 1.0 + 0.05 * sin(in.uv.y * 28.0) * sin(in.uv.y * 3.0);

    let lit = base * banding * (ambient + wrapped * (1.0 - ambient))
        + vec3<f32>(specular)
        + base * rim;
    return vec4<f32>(lit, 1.0);
}
