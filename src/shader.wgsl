// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vert_pos: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var positions: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
        vec2<f32>(-1, -1),
        vec2<f32>(1, -1),
        vec2<f32>(-1, 1),
        vec2<f32>(1, 1),
    );
    var pos: vec2<f32> = positions[in_vertex_index];
    return VertexOutput(
        vec4<f32>(pos, 0.0, 1.0),
        pos/2.0 + vec2<f32>(0.5),
    );
} 

// Fragment shader

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    var K = vec4<f32>(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    var p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn mandelbrot(c: vec2<f64>, max_its: u32) -> u32 {
    var z = vec2<f64>(0.0);

    for (var i = 0u; i < max_its; i++) {
        if (dot(z, z) >= 4.0) {
            return i;
        }
        var z_conj = vec2<f64>(z.x, -z.y);
        z = vec2<f64>(
            z.x*z.x - z.y*z.y + c.x,
            2.0*z.x*z.y + c.y
        );
    }

    return max_its;
}

fn pixel_color(its: u32, max_its: u32) -> vec3<f32> {
    if its == max_its {
        return vec3<f32>(0.0);
    } else {
        var h = f32(its)/f32(max_its);
        var hsv = vec3<f32>(h, 1.0, f32(its < max_its));
        return hsv2rgb(hsv);
    }
}

fn uv2coord(uv: vec2<f64>) -> vec2<f64> {
    var width = uni.aspect_ratio * uni.height;
    return vec2<f64>(
        uni.min_x + uv.x * width,
        uni.min_y + uv.y * uni.height,
    );
}

struct MandelbrotUniform {
    min_x: f64,
    min_y: f64,
    height: f64,
    aspect_ratio: f64,
    max_its: u32,
    padding: u32,
}

@group(0) @binding(0)
var<uniform> uni: MandelbrotUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // var xrange = vec2<f32>(-0.749488, -0.7492405);
    // var yrange = vec2<f32>(0.031567533, 0.03170943);
    var coord = uv2coord(vec2<f64>(in.vert_pos));
    var max_its: u32 = uni.max_its;
    var m = mandelbrot(coord, max_its);
    return vec4<f32>(pixel_color(m, max_its), 0.0);
}
