use crate::gl_bindings as gl;
use anyhow::{Result, anyhow};
use std::ffi::{CStr, CString};
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

pub fn compile_shader(vert_source: &str, frag_source: &str) -> Result<u32> {
    unsafe {
        let program = gl::CreateProgram();

        let vert_shader = gl::CreateShader(gl::VERTEX_SHADER);
        let vert_c_str = CString::new(vert_source)?;
        gl::ShaderSource(vert_shader, 1, &vert_c_str.as_ptr(), std::ptr::null());
        gl::CompileShader(vert_shader);
        check_shader_compile(vert_shader, "vertex")?;

        let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        let frag_c_str = CString::new(frag_source)?;
        gl::ShaderSource(frag_shader, 1, &frag_c_str.as_ptr(), std::ptr::null());
        gl::CompileShader(frag_shader);
        check_shader_compile(frag_shader, "fragment")?;

        gl::AttachShader(program, vert_shader);
        gl::AttachShader(program, frag_shader);
        gl::LinkProgram(program);
        check_program_link(program)?;

        gl::DeleteShader(vert_shader);
        gl::DeleteShader(frag_shader);

        Ok(program)
    }
}


fn check_shader_compile(shader: u32, shader_type: &str) -> Result<()> {
    unsafe {
        let mut status = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
        if status == gl::FALSE as i32 {
            let mut log_length = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut log_length);
            let mut log = vec![0u8; log_length as usize];
            gl::GetShaderInfoLog(
                shader,
                log_length,
                std::ptr::null_mut(),
                log.as_mut_ptr() as *mut i8,
            );
            let log_str = CStr::from_ptr(log.as_ptr() as *const i8).to_string_lossy();
            return Err(anyhow!(
                "{} shader compilation failed: {}",
                shader_type,
                log_str
            ));
        }
    }
    Ok(())
}

fn check_program_link(program: u32) -> Result<()> {
    unsafe {
        let mut status = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);
        if status == gl::FALSE as i32 {
            let mut log_length = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut log_length);
            let mut log = vec![0u8; log_length as usize];
            gl::GetProgramInfoLog(
                program,
                log_length,
                std::ptr::null_mut(),
                log.as_mut_ptr() as *mut i8,
            );
            let log_str = CStr::from_ptr(log.as_ptr() as *const i8).to_string_lossy();
            return Err(anyhow!("Program linking failed: {}", log_str));
        }
    }
    Ok(())
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

    // Remove precision directives that will be added automatically
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
