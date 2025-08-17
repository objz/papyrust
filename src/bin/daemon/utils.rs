use crate::gl_bindings as gl;
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

pub fn check_gl_error(context: &str) {
    unsafe {
        let error = gl::GetError();
        if error != gl::NO_ERROR {
            let error_str = match error {
                gl::INVALID_ENUM => "GL_INVALID_ENUM",
                gl::INVALID_VALUE => "GL_INVALID_VALUE",
                gl::INVALID_OPERATION => "GL_INVALID_OPERATION",
                gl::OUT_OF_MEMORY => "GL_OUT_OF_MEMORY",
                _ => "Unknown error",
            };
            tracing::error!(
                event = "gl_error",
                context = %context,
                error = %error_str,
                error_code = error,
                "OpenGL error detected"
            );
        }
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

pub fn prepare_shader_source(raw_shader: &str) -> String {
    let mut version_directive: Option<&str> = None;
    let mut body_lines = Vec::new();

    for line in raw_shader.lines() {
        let trimmed = line.trim_start();
        if version_directive.is_none() && trimmed.starts_with("#version") {
            version_directive = Some(line);
        } else {
            body_lines.push(line);
        }
    }

    body_lines.retain(|l| {
        let t = l.trim_start();
        !(t.starts_with("precision ") && t.ends_with("float;"))
    });

    let mut frag_source = String::new();
    if let Some(v) = version_directive {
        frag_source.push_str(v);
        frag_source.push('\n');
    }

    frag_source.push_str(
        r#"
        #ifdef GL_ES
          #ifdef GL_FRAGMENT_PRECISION_HIGH
            precision highp float;
          #else
            precision mediump float;
          #endif
        #endif
        "#,
    );

    frag_source.push_str(&body_lines.join("\n"));
    frag_source
}
