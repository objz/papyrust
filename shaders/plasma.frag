#ifdef GL_ES
precision mediump float;
#endif

uniform float time;
uniform vec2 resolution;
varying vec2 texCoords;

void main() {
    vec2 uv = texCoords;
    vec2 p = (uv - 0.5) * vec2(resolution.x/resolution.y, 1.0) * 2.0;

    float v1 = sin(p.x * 3.0 + time);
    float v2 = sin((p.x * cos(time*0.7) + p.y * sin(time*0.3)) * 4.0);
    float v3 = sin(length(p) * 5.0 - time * 1.5);

    float v = (v1 + v2 + v3) / 3.0;

    vec3 col = vec3(
        0.5 + 0.5 * sin(3.0 + v * 3.0),
        0.5 + 0.5 * sin(1.0 + v * 3.0),
        0.5 + 0.5 * sin(5.0 + v * 3.0)
    );

    gl_FragColor = vec4(col, 1.0);
}
