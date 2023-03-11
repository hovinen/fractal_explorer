struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec2<f32>,
}

struct Uniform {
    transform: mat3x3<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniform;

fn mandelbrot_iterations(position: vec3<f32>) -> f32 {
    let position = u.transform * position;
    let c = vec2(position.x, position.y);
    var z = vec2(0.0, 0.0);
    var i = 0.0;
    while (i <= 1.0) {
        z = vec2(
            z.x * z.x - z.y * z.y + c.x,
            2.0 * z.y * z.x + c.y
        );

        if (length(z) > 4.0) {
            break;
        }

        i += 0.001;
    }

    if i > 1.0 {
        return 0.0;
    } else {
        return i;
    }
}

@fragment
fn mandelbrot(in: VertexOutput) -> @location(0) vec4<f32> {
    let position = vec3(in.position, 1.0);
    return vec4(vec3(mandelbrot_iterations(position)), 1.0);
}

@group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

@compute
@workgroup_size(1)
fn apply_uniform() {
    let v_out = u.transform * v;
    v = v_out;
}

@compute
@workgroup_size(1)
fn run_mandelbrot_iteration() {
    let i = mandelbrot_iterations(v);
    v = vec3(i, 0.0, 0.0);
}
