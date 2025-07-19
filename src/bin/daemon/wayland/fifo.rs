use anyhow::{anyhow, Result};
use std::os::unix::io::{AsRawFd, RawFd};



const N_SAMPLES: usize = 44100 / 25;

#[derive(Debug)]
pub struct StereoSample {
    pub left: Vec<i16>,
    pub right: Vec<i16>,
}

impl StereoSample {
    fn new() -> Self {
        Self {
            left: vec![0; N_SAMPLES],
            right: vec![0; N_SAMPLES],
        }
    }
}


pub struct FifoReader {
    pub fd: RawFd,
}

impl FifoReader {
    pub fn new(fifo_path: &str) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(fifo_path)?;

        Ok(Self {
            fd: file.as_raw_fd(),
        })
    }

    pub fn read_sample(&mut self) -> Result<Option<StereoSample>> {
        let mut buffer = vec![0u8; N_SAMPLES * 4];

        let bytes_read = unsafe {
            libc::read(
                self.fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if bytes_read < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                return Ok(None);
            }
            return Err(anyhow!("Failed to read from FIFO: {}", errno));
        }

        if bytes_read == 0 {
            return Ok(None);
        }

        let samples_read = bytes_read as usize / 4;
        let mut stereo = StereoSample::new();

        for i in 0..samples_read.min(N_SAMPLES / 2) {
            let base = i * 4;
            if base + 3 < buffer.len() {
                stereo.left[i] = i16::from_le_bytes([buffer[base], buffer[base + 1]]);
                stereo.right[i] = i16::from_le_bytes([buffer[base + 2], buffer[base + 3]]);
            }
        }

        Ok(Some(stereo))
    }
}
