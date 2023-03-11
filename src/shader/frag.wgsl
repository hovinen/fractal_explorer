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

fn mul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn inv(z: vec2<f32>) -> vec2<f32> {
    return vec2(z.x, -z.y) / (z.x * z.x + z.y * z.y);
}

fn eval_poly(z: vec2<f32>, coeffs: vec4<f32>) -> vec2<f32> {
    var f_z = vec2(coeffs[0], 0.0);
    f_z = mul(f_z, z);
    f_z += vec2(coeffs[1], 0.0);
    f_z = mul(f_z, z);
    f_z += vec2(coeffs[2], 0.0);
    f_z = mul(f_z, z);
    return f_z + vec2(coeffs[3], 0.0);
}

let ITERATIONS = 100;
let COEFFS = vec4<f32>(1.0, 0.0, 0.0, -1.0);
let DERIVATIVE_COEFFS = vec4<f32>(0.0, 3.0, 0.0, 0.0);

fn newton_iterate(position: vec3<f32>) -> vec2<f32> {
    var z = vec2(position.x, position.y);
    for (var i: i32 = 0; i < ITERATIONS; i += 1) {
        let f_z = eval_poly(z, COEFFS);
        let fp_z = eval_poly(z, DERIVATIVE_COEFFS);
        let fp_z_inv = inv(fp_z);
        z = z - mul(f_z, fp_z_inv);
    }

    return z;
}

fn distance_sq(a: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = a - b;
    return d.x * d.x + d.y * d.y;
}

let EPSILON = 1e-4;
let ROOTS = array<vec2<f32>, 3>(
    vec2<f32>(1.0, 0.0),
    vec2<f32>(-0.5, 0.866025),
    vec2<f32>(-0.5, -0.866025),
);
let COLOURS = array<vec3<f32>, 3>(
    vec3<f32>(1.0, 0.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(0.0, 0.0, 1.0),
);

fn point_colour(z: vec2<f32>) -> vec3<f32> {
    var colour = vec3(0.0);
    if distance_sq(z, ROOTS[0]) < EPSILON {
        colour = COLOURS[0];
    }
    if distance_sq(z, ROOTS[1]) < EPSILON {
        colour = COLOURS[1];
    }
    if distance_sq(z, ROOTS[2]) < EPSILON {
        colour = COLOURS[2];
    }
    return colour;
}

@fragment
fn newton(in: VertexOutput) -> @location(0) vec4<f32> {
    let position = u.transform * vec3(in.position, 1.0);
    return vec4(point_colour(newton_iterate(position)), 1.0);
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

@compute
@workgroup_size(1)
fn run_eval_poly() {
    let result = eval_poly(vec2(v.x, v.y), COEFFS);
    v = vec3(result, 0.0);
}

@compute
@workgroup_size(1)
fn run_eval_poly_df() {
    let result = eval_poly(vec2(v.x, v.y), DERIVATIVE_COEFFS);
    v = vec3(result, 0.0);
}

@compute
@workgroup_size(1)
fn run_inv() {
    let result = inv(vec2(v.x, v.y));
    v = vec3(result, 0.0);
}

@compute
@workgroup_size(1)
fn run_newton() {
    let result = newton_iterate(v);
    v = vec3(result, 0.0);
}
