use anyhow::{anyhow, Result};
use memmap2::MmapMut;
use nix::sys::memfd;
use std::ffi::CStr;
use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, AsFd};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool},
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

struct CaptureState {
    shm: Option<wl_shm::WlShm>,
    screencopy_manager: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    output: Option<wl_output::WlOutput>,
    
    // Frame state
    buffer_done: bool,
    buffer_ready: bool,
    width: u32,
    height: u32,
    stride: u32,
    format: wl_shm::Format,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<wl_shm::WlShm, ()> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<wl_output::WlOutput, ()> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_output::WlOutput,
        _event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for CaptureState {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        _event: zwlr_screencopy_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {}
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for CaptureState {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<CaptureState>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                state.width = width;
                state.height = height;
                state.stride = stride;
                if let WEnum::Value(f) = format {
                    state.format = f;
                }
                state.buffer_done = true;
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                state.buffer_ready = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                eprintln!("Screencopy failed!");
            }
            _ => {}
        }
    }
}

pub struct Capturer {
    // conn: Connection, // Not strictly needed to store if we have event_queue
    event_queue: wayland_client::EventQueue<CaptureState>,
    state: CaptureState,
}

pub fn setup() -> Result<Capturer> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init::<CaptureState>(&conn)?;
    let qh = event_queue.handle();

    let mut state = CaptureState {
        shm: None,
        screencopy_manager: None,
        output: None,
        buffer_done: false,
        buffer_ready: false,
        width: 0,
        height: 0,
        stride: 0,
        format: wl_shm::Format::Argb8888,
    };

    // Bind globals
    state.shm = globals.bind(&qh, 1..=1, ()).ok();
    state.screencopy_manager = globals.bind(&qh, 1..=3, ()).ok();
    
    let output_global = globals.contents().clone_list().into_iter()
        .find(|g| g.interface == "wl_output")
        .ok_or_else(|| anyhow!("No output found"))?;
        
    state.output = Some(globals.registry().bind::<wl_output::WlOutput, _, _>(
        output_global.name,
        1,
        &qh,
        ()
    ));

    if state.shm.is_none() || state.screencopy_manager.is_none() {
        return Err(anyhow!("Missing required Wayland globals (wl_shm or zwlr_screencopy_manager_v1)"));
    }

    event_queue.roundtrip(&mut state)?;

    Ok(Capturer {
        event_queue,
        state,
    })
}

impl Capturer {
    pub async fn capture_frame(&mut self) -> Result<(Vec<u8>, u32, u32, u32)> {
        // We need to clone the proxies so we don't hold a borrow on self.state
        // Proxies in wayland-client are cheap to clone (just an ID and a pointer)
        let manager = self.state.screencopy_manager.as_ref().unwrap().clone();
        let output = self.state.output.as_ref().unwrap().clone();
        let shm = self.state.shm.as_ref().unwrap().clone();
        let qh = self.event_queue.handle();

        // 1. Create a frame request
        let frame = manager.capture_output(0, &output, &qh, ());

        // 2. Wait for Buffer event
        self.state.buffer_done = false;
        self.state.buffer_ready = false;
        
        while !self.state.buffer_done {
            self.event_queue.blocking_dispatch(&mut self.state)?;
        }

        // 3. Create SHM buffer
        let size = (self.state.stride * self.state.height) as usize;
        
        let fd = memfd::memfd_create(
            CStr::from_bytes_with_nul(b"hyprhue-shm\0")?,
            memfd::MemFdCreateFlag::MFD_CLOEXEC,
        )?;
        
        let file = File::from(fd);
        file.set_len(size as u64)?;
        
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        
        let pool = shm.create_pool(file.as_fd(), size as i32, &qh, ());
        
        let buffer = pool.create_buffer(
            0,
            self.state.width as i32,
            self.state.height as i32,
            self.state.stride as i32,
            self.state.format,
            &qh,
            (),
        );

        // 4. Copy frame
        frame.copy(&buffer);

        // 5. Wait for Ready event
        while !self.state.buffer_ready {
            self.event_queue.blocking_dispatch(&mut self.state)?;
        }
        
        // 6. Copy data out
        let data = mmap.to_vec();
        let width = self.state.width;
        let height = self.state.height;
        let stride = self.state.stride;
        
        // Cleanup
        buffer.destroy();
        pool.destroy();
        frame.destroy();
        
        Ok((data, width, height, stride))
    }
}
