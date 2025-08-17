fn lanczos(x: f32) -> f32 {
    if abs(x) < 1e-6 {
        return 1.0;
    }
    if abs(x) >= 3.0 {
        return 0.0;
    }
    let pi_x = 3.14159265359 * x;
    return 3.0 * sin(pi_x) * sin(pi_x / 3.0) / (pi_x * pi_x);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_coords = input.tex_coords;
    let scaled_coords = tex_coords * uniforms.input_size;
    let base_coords = floor(scaled_coords - 0.5) + 0.5;
    let f = scaled_coords - base_coords;
    
    var result = vec4<f32>(0.0);
    var weight_sum = 0.0;
    
    for (var i = -2; i <= 3; i++) {
        for (var j = -2; j <= 3; j++) {
            let sample_coords = (base_coords + vec2<f32>(f32(i), f32(j))) / uniforms.input_size;
            let weight_x = lanczos(f32(i) - f.x);
            let weight_y = lanczos(f32(j) - f.y);
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
    
    let center = textureSample(input_texture, texture_sampler, tex_coords);
    let sharpened = result + (result - center) * uniforms.sharpening;
    
    return clamp(sharpened, vec4<f32>(0.0), vec4<f32>(1.0));
}
