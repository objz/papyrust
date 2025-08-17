use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn sleep_millis(millis: u64) {
    if millis > 0 {
        thread::sleep(Duration::from_millis(millis));
    }
}

pub fn default_shader() -> &'static str {
    r#"
#ifdef GL_ES
precision highp float;
#endif

uniform sampler2D u_media;
uniform vec2 u_resolution;
uniform float u_time;

varying vec2 texCoords;

void main() {
    vec2 uv = texCoords;
    
    // High-quality scaling with subtle animation
    float scale = 1.0 + 0.005 * sin(u_time * 1.5);
    vec2 center = vec2(0.5);
    uv = (uv - center) * scale + center;
    
    // Ensure UV coordinates stay within bounds
    uv = clamp(uv, 0.0, 1.0);
    
    // Sample texture with high precision
    vec4 color = texture2D(u_media, uv);
    
    // Preserve original color fidelity
    gl_FragColor = color;
}
"#
}

pub fn vertex_shader() -> &'static str {
    r#"
#version 100
attribute highp vec2 datIn;
attribute highp vec2 texIn;
varying highp vec2 texCoords;

void main() {
    texCoords = texIn;
    gl_Position = vec4(datIn, 0.0, 1.0);
}
"#
}
