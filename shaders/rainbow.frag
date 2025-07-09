#version 100
precision mediump float;

uniform float time;
uniform vec2 resolution;

void main() {
    vec2 uv = gl_FragCoord.xy / resolution;

    float wave = 0.5 + 0.5 * sin(uv.x * 10.0 + time);

    gl_FragColor = vec4(uv.x, uv.y, wave, 1.0);
}
