#version 330 core

// Fragment shader for particles
uniform float u_time;
uniform vec2 u_resolution;
in vec2 v_texCoord;
out vec4 fragColor;

void main() {
    vec2 uv = v_texCoord;
    
    // Simple animated colors
    float r = sin(u_time + uv.x * 5.0) * 0.5 + 0.5;
    float g = cos(u_time + uv.y * 3.0) * 0.5 + 0.5;
    float b = sin(u_time + (uv.x + uv.y) * 2.0) * 0.5 + 0.5;
    
    fragColor = vec4(r, g, b, 1.0);
}