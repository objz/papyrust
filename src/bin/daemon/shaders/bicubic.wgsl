fn cubic(t: f32) -> f32 {
    let a = -0.5;
    let t2 = t * t;
    let t3 = t2 * t;
    
    if t <= 1.0 {
        return (a + 2.0) * t3 - (a + 3.0) * t2 + 1.0;
    } else if t <= 2.0 {
        return a * t3 - 5.0 * a * t2 + 8.0 * a * t - 4.0 * a;
    } else {
        return 0.0;
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_coords = input.tex_coords;
    let scaled_coords = tex_coords * uniforms.input_size;
    let base_coords = floor(scaled_coords - 0.5) + 0.5;
    let f = scaled_coords - base_coords;
    
    var result = vec4<f32>(0.0);
    
    for (var i = -1; i <= 2; i++) {
        for (var j = -1; j <= 2; j++) {
            let sample_coords = (base_coords + vec2<f32>(f32(i), f32(j))) / uniforms.input_size;
            let weight_x = cubic(abs(f32(i) - f.x));
            let weight_y = cubic(abs(f32(j) - f.y));
            let weight = weight_x * weight_y;
            
            if sample_coords.x >= 0.0 && sample_coords.x <= 1.0 && 
               sample_coords.y >= 0.0 && sample_coords.y <= 1.0 {
                result += textureSample(input_texture, texture_sampler, sample_coords) * weight;
            }
        }
    }
    
    return clamp(result, vec4<f32>(0.0), vec4<f32>(1.0));
}
