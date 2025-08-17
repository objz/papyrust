use anyhow::Result;

pub trait WaylandSurface {
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;
    fn get_output_name(&self) -> &str;
}
