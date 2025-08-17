fn mitchell(x: f32) -> f32 {
    let b = 1.0 / 3.0;
    let c = 1.0 / 3.0;
    let ax = abs(x);
    
    if ax < 1.0 {
        return ((12.0 - 9.0 * b - 6.0 * c) * ax * ax * ax + 
                (-18.0 + 12.0 * b + 6.0 * c) * ax * ax + 
                (6.0 - 2.0 * b)) / 6.0;
    } else if ax < 2.0 {
        return ((-b - 6.0 * c) * ax * ax * ax + 
                (6.0 * b + 30.0 * c) * ax * ax + 
                (-12.0 * b - 48.0 * c) * ax + 
                (8.0 * b + 24.0 * c)) / 6.0;
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
    var weight_sum = 0.0;
    
    for (var i = -1; i <= 2; i++) {
        for (var j = -1; j <= 2; j++) {
            let sample_coords = (base_coords + vec2<f32>(f32(i), f32(j))) / uniforms.input_size;
            let weight_x = mitchell(f32(i) - f.x);
            let weight_y = mitchell(f32(j) - f.y);
            let weight = weight_x * weight_y;
            
            if sample_coords.x >= 0.0 && sample_coords.x <= 1.0 && 
               sample_coords.y >= 0.0 && sample_coords.y <= 1.0 {
                result += textureSample(input_texture, texture_sampler, sample_coords) * weight;
                weight_sum += weight;
            }
        }
    }
    
    if weight_sum > 0.0 {
        result /= weight_sum;
    }
    
    return clamp(result, vec4<f32>(0.0), vec4<f32>(1.0));
}
