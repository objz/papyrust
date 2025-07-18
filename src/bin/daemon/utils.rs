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
precision mediump float;
#endif

uniform sampler2D u_media;
uniform vec2 u_resolution;
uniform float u_time;

varying vec2 texCoords;

void main() {
    vec2 uv = texCoords;
    float scale = 1.0 + 0.01 * sin(u_time * 2.0);
    uv = (uv - 0.5) * scale + 0.5;
    vec4 color = texture2D(u_media, uv);
    gl_FragColor = color;
}
"#
}

pub fn vertex_shader() -> &'static str {
    r#"
#version 100
attribute vec2 datIn;
attribute vec2 texIn;
varying vec2 texCoords;

void main() {
    texCoords = texIn;
    gl_Position = vec4(datIn, 0.0, 1.0);
}
"#
}
