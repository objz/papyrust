use crate::gl_bindings as gl;
use anyhow::{Result, anyhow};
use std::ffi::{CStr, CString};

pub struct GlTexture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

impl GlTexture {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
        }
        
        Ok(Self { id: texture, width, height })
    }

    pub fn from_rgba_data(width: u32, height: u32, data: &[u8], with_mipmaps: bool) -> Result<Self> {
        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

            if with_mipmaps {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

                let mut max_anisotropy = 0.0f32;
                gl::GetFloatv(0x84FE, &mut max_anisotropy);
                if max_anisotropy > 1.0 {
                    let anisotropy = max_anisotropy.min(16.0);
                    gl::TexParameterf(gl::TEXTURE_2D, 0x84FE, anisotropy);
                }
            } else {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            }

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );

            if with_mipmaps {
                gl::GenerateMipmap(gl::TEXTURE_2D);
            }
        }

        Ok(Self { id: texture, width, height })
    }

    pub fn update_data(&self, data: &[u8]) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.id);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                self.width as i32,
                self.height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
        }
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.id);
        }
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.id);
        }
    }
}

pub struct GlProgram {
    pub id: u32,
}

impl GlProgram {
    pub fn new(vert_source: &str, frag_source: &str) -> Result<Self> {
        unsafe {
            let program = gl::CreateProgram();
            crate::utils::check_gl_error("CreateProgram");

            let vert_shader = Self::compile_shader(gl::VERTEX_SHADER, vert_source)?;
            let frag_shader = Self::compile_shader(gl::FRAGMENT_SHADER, frag_source)?;

            gl::AttachShader(program, vert_shader);
            gl::AttachShader(program, frag_shader);
            crate::utils::check_gl_error("AttachShader");
            
            gl::LinkProgram(program);
            crate::utils::check_gl_error("LinkProgram");
            Self::check_program_link(program)?;

            gl::DeleteShader(vert_shader);
            gl::DeleteShader(frag_shader);
            crate::utils::check_gl_error("DeleteShader");

            tracing::debug!(
                event = "shader_compiled",
                program,
                "Successfully compiled and linked shader program"
            );

            Ok(Self { id: program })
        }
    }

    fn compile_shader(shader_type: u32, source: &str) -> Result<u32> {
        unsafe {
            let shader = gl::CreateShader(shader_type);
            crate::utils::check_gl_error("CreateShader");
            
            let c_str = CString::new(source)?;
            gl::ShaderSource(shader, 1, &c_str.as_ptr(), std::ptr::null());
            gl::CompileShader(shader);
            crate::utils::check_gl_error("CompileShader");
            
            let shader_type_name = match shader_type {
                gl::VERTEX_SHADER => "vertex",
                gl::FRAGMENT_SHADER => "fragment",
                _ => "unknown",
            };
            Self::check_shader_compile(shader, shader_type_name)?;
            
            Ok(shader)
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

    pub fn use_program(&self) {
        unsafe {
            gl::UseProgram(self.id);
        }
    }

    pub fn get_uniform_location(&self, name: &str) -> i32 {
        unsafe {
            let c_name = CString::new(name).unwrap();
            gl::GetUniformLocation(self.id, c_name.as_ptr())
        }
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
