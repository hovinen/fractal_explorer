struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec2<f32>,
}

@vertex
fn main(@location(0) position: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.position = position;
    return output;
}
