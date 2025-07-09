#version 330 core

// Fragment shader for particle effects
in vec2 fragCoord;
out vec4 fragColor;

uniform float time;
uniform vec2 resolution;

void main() {
    vec2 uv = fragCoord / resolution.xy;
    
    // Simple particle effect
    float r = 0.3 + 0.2 * sin(time * 2.0 + uv.x * 10.0);
    float g = 0.5 + 0.3 * cos(time * 1.5 + uv.y * 8.0);
    float b = 0.8 + 0.2 * sin(time * 3.0 + (uv.x + uv.y) * 5.0);
    
    fragColor = vec4(r, g, b, 1.0);
}