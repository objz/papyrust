@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_coords = input.tex_coords;
    let texel_size = 1.0 / uniforms.input_size;
    
    let b = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(-texel_size.x, -texel_size.y));
    let d = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(0.0, -texel_size.y));
    let f = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(texel_size.x, -texel_size.y));
    let h = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(-texel_size.x, 0.0));
    let i = textureSample(input_texture, texture_sampler, tex_coords);
    let j = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(texel_size.x, 0.0));
    let l = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(-texel_size.x, texel_size.y));
    let n = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(0.0, texel_size.y));
    let p = textureSample(input_texture, texture_sampler, tex_coords + vec2<f32>(texel_size.x, texel_size.y));
    
    let min_g = min(min(min(d.g, h.g), min(i.g, j.g)), n.g);
    let max_g = max(max(max(d.g, h.g), max(i.g, j.g)), n.g);
    
    let w = 1.0 - saturate((max_g - min_g) / max(max_g, 1e-6));
    
    let dir_horizontal = abs(h.g + j.g - 2.0 * i.g);
    let dir_vertical = abs(d.g + n.g - 2.0 * i.g);
    
    var result: vec4<f32>;
    if dir_horizontal < dir_vertical {
        result = mix(d, n, 0.5);
    } else {
        result = mix(h, j, 0.5);
    }
    
    let sharpened = i + (i - result) * uniforms.sharpening * w;
    
    return mix(i, sharpened, w);
}
