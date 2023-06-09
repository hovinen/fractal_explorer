struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec2<f32>,
}

struct Uniform {
    transform: mat3x3<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniform;

fn mandelbrot_iterations(c: vec2<f32>) -> f32 {
    var z = vec2(0.0, 0.0);
    var z2 = vec2(0.0, 0.0);
    var i = 0.0;
    while (i <= 1.0) {
        z = vec2(z2.x - z2.y + c.x, 2.0 * z.x * z.y + c.y);
        z2 = vec2(z.x * z.x, z.y * z.y);

        if (z2.x + z2.y > 4.0) {
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
    let position = u.transform * vec3(in.position, 1.0);
    return vec4(vec3(mandelbrot_iterations(vec2(position.x, position.y))), 1.0);
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

const ITERATIONS = 100;
const COEFFS = vec4<f32>(1.0, 0.0, 0.0, -1.0);
const DERIVATIVE_COEFFS = vec4<f32>(0.0, 3.0, 0.0, 0.0);

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

const EPSILON = 1e-4;
const ROOTS = array<vec2<f32>, 3>(
    vec2<f32>(1.0, 0.0),
    vec2<f32>(-0.5, 0.866025),
    vec2<f32>(-0.5, -0.866025),
);
const COLOURS = array<vec3<f32>, 3>(
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
