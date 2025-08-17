use anyhow::{Result, anyhow};
use image as img_crate;
use crate::gl_utils::GlTexture;
use crate::media::{MediaHandler, BaseMediaHandler};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ImageHandler {
    base: BaseMediaHandler,
    loading_state: Arc<Mutex<LoadingState>>,
}

#[derive(Debug)]
enum LoadingState {
    Loading,
    DataReady {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    TextureCreated,
    Error(String),
}

impl ImageHandler {
    pub fn new(path: &str, shader_path: Option<&str>) -> Result<Self> {
        tracing::info!(
            event = "image_create",
            path = %path,
            shader = shader_path.unwrap_or("default"),
            "Creating image handler"
        );

        let base = BaseMediaHandler::new_with_shader(shader_path)?;
        let loading_state = Arc::new(Mutex::new(LoadingState::Loading));
        
        let path_clone = path.to_string();
        let loading_state_clone = loading_state.clone();
        
        thread::spawn(move || {
            match Self::load_image_data(&path_clone) {
                Ok((width, height, data)) => {
                    tracing::debug!(
                        event = "image_data_loaded",
                        width,
                        height,
                        path = %path_clone,
                        "Image data loaded successfully"
                    );
                    if let Ok(mut state) = loading_state_clone.lock() {
                        *state = LoadingState::DataReady { width, height, data };
                    }
                }
                Err(e) => {
                    tracing::error!(
                        event = "image_load_error",
                        error = %e,
                        path = %path_clone,
                        "Failed to load image"
                    );
                    if let Ok(mut state) = loading_state_clone.lock() {
                        *state = LoadingState::Error(e.to_string());
                    }
                }
            }
        });

        Ok(Self { 
            base,
            loading_state,
        })
    }

    fn load_image_data(path: &str) -> Result<(u32, u32, Vec<u8>)> {
        tracing::info!(event = "texture_load", path = %path, "Loading image data");

        let img = img_crate::open(path)
            .map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (width, height) = (img.width(), img.height());

        tracing::debug!(event = "image_info", width, height, "Image decoded");

        Ok((width, height, rgba.into_raw()))
    }

    fn check_loading_state(&mut self) -> bool {
        if let Ok(mut state) = self.loading_state.lock() {
            match std::mem::replace(&mut *state, LoadingState::Loading) {
                LoadingState::Loading => {
                    *state = LoadingState::Loading;
                    false
                }
                LoadingState::DataReady { width, height, data } => {
                    match GlTexture::from_rgba_data(width, height, &data, true) {
                        Ok(texture) => {
                            self.base.dimensions = (texture.width, texture.height);
                            self.base.texture = Some(texture);
                            self.base.has_new_frame = true;
                            *state = LoadingState::TextureCreated;
                            
                            tracing::debug!(
                                event = "texture_created",
                                width,
                                height,
                                "Texture created successfully"
                            );
                            true
                        }
                        Err(e) => {
                            tracing::error!(
                                event = "texture_create_error",
                                error = %e,
                                "Failed to create texture from loaded data"
                            );
                            *state = LoadingState::Error(e.to_string());
                            false
                        }
                    }
                }
                LoadingState::TextureCreated => {
                    *state = LoadingState::TextureCreated;
                    false
                }
                LoadingState::Error(e) => {
                    *state = LoadingState::Error(e);
                    false
                }
            }
        } else {
            false
        }
    }
}

impl MediaHandler for ImageHandler {
    fn get_texture(&self) -> Option<&GlTexture> {
        self.base.texture.as_ref()
    }

    fn get_dimensions(&self) -> (u32, u32) {
        self.base.dimensions
    }

    fn update(&mut self) -> Result<bool> {
        let loaded = self.check_loading_state();
        if loaded && self.base.has_new_frame {
            self.base.has_new_frame = false;
            return Ok(true);
        }
        Ok(false)
    }

    fn has_new_frame(&self) -> bool {
        self.base.has_new_frame
    }

    fn get_shader_program(&self) -> &crate::gl_utils::GlProgram {
        &self.base.shader_program
    }
}
