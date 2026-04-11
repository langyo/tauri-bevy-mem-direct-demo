use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use shared::event::{NamedEvent, FRAME_EVENT_NAME};
use shared::protocol::ToDesktop;
use shared::shm::{
    DualControl, FrameHeader, ShmHandle, BUFFER_SIZE, DUAL_CONTROL_SIZE, FRAME_HEADER_SIZE,
    SHM_MAX_SIZE, SHM_NAME,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

const FPS_REPORT_INTERVAL_SECS: f64 = 0.5;

#[derive(Resource)]
pub struct FpsTracker {
    frames_since_report: u32,
    last_report_time: f64,
    last_fps: f64,
    total_frames: u64,
}

#[derive(Resource)]
pub struct CaptureResolution {
    pub width: u32,
    pub height: u32,
}

impl CaptureResolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Resource, Default)]
pub struct ResolutionDirty(pub bool);

#[derive(Resource)]
pub struct CaptureImageHandle(pub Handle<Image>);

#[derive(Resource)]
pub struct ShmWriter(Arc<std::sync::Mutex<ShmHandle>>);

#[derive(Resource)]
pub struct FrameEvent(NamedEvent);

#[derive(Component)]
pub struct ReadbackMarker;

pub fn setup_capture_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<&mut Camera, With<crate::camera::MainCamera>>,
    res: Res<CaptureResolution>,
) {
    let mut shm =
        ShmHandle::create(SHM_NAME, SHM_MAX_SIZE).expect("Failed to create shared memory");
    eprintln!(
        "[renderer] shared memory created: {} bytes (dual-buffer)",
        SHM_MAX_SIZE
    );

    {
        let slice = shm.slice_mut();
        let ctrl = DualControl::as_bytes(slice);
        ctrl.write_index.store(0, Ordering::Release);
        ctrl.ready_index.store(1, Ordering::Release);
    }

    let size = Extent3d {
        width: res.width,
        height: res.height,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC;

    let handle = images.add(image);
    commands.insert_resource(CaptureImageHandle(handle.clone()));

    for mut camera in query.iter_mut() {
        camera.target = RenderTarget::Image(handle.clone());
    }

    commands.insert_resource(ShmWriter(Arc::new(std::sync::Mutex::new(shm))));

    let frame_event = NamedEvent::create(FRAME_EVENT_NAME).expect("Failed to create frame event");
    commands.insert_resource(FrameEvent(frame_event));

    commands.spawn((Readback::texture(handle), ReadbackMarker));

    eprintln!(
        "[renderer] capture image ready: {}x{}",
        res.width, res.height
    );
}

pub fn rebuild_capture_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    res: Res<CaptureResolution>,
    mut handle_res: ResMut<CaptureImageHandle>,
    mut query: Query<&mut Camera, With<crate::camera::MainCamera>>,
    mut dirty: ResMut<ResolutionDirty>,
    readback_query: Query<Entity, With<ReadbackMarker>>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let size = Extent3d {
        width: res.width,
        height: res.height,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC;

    let old_id = handle_res.0.id();
    let new_handle = images.add(image);
    handle_res.0 = new_handle.clone();

    images.remove(old_id);

    for mut camera in query.iter_mut() {
        camera.target = RenderTarget::Image(new_handle.clone());
    }

    for entity in readback_query.iter() {
        commands.entity(entity).despawn();
    }

    commands.spawn((Readback::texture(new_handle), ReadbackMarker));
    eprintln!(
        "[renderer] rebuilt capture image: {}x{}",
        res.width, res.height
    );
}

static mut FRAME_COUNT: u64 = 0;

pub fn on_readback_complete(
    trigger: Trigger<ReadbackComplete>,
    res: Res<CaptureResolution>,
    shm: Res<ShmWriter>,
    frame_event: Res<FrameEvent>,
) {
    let data = &trigger.event().0;
    if data.is_empty() {
        return;
    }

    let w = res.width;
    let h = res.height;
    let raw_row = w * 4;
    let aligned_row = ((raw_row + 255) / 256) * 256;

    let mut shm_guard = shm.0.lock().unwrap();
    let full_slice = shm_guard.slice_mut();

    let write_idx = {
        let ctrl = DualControl::as_bytes(full_slice);
        ctrl.write_index.load(Ordering::Acquire)
    };

    let buf_offset = DUAL_CONTROL_SIZE + write_idx as usize * BUFFER_SIZE;
    let buf = &mut full_slice[buf_offset..buf_offset + BUFFER_SIZE];

    let bytes_written = if aligned_row != raw_row {
        let mut dst = FRAME_HEADER_SIZE;
        for y in 0..h {
            let src = (y * aligned_row) as usize;
            let end = src + raw_row as usize;
            if end > data.len() || dst + raw_row as usize > buf.len() {
                break;
            }
            buf[dst..dst + raw_row as usize].copy_from_slice(&data[src..end]);
            dst += raw_row as usize;
        }
        (raw_row * h) as usize
    } else {
        let copy_len = data.len().min(buf.len() - FRAME_HEADER_SIZE);
        buf[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + copy_len].copy_from_slice(&data[..copy_len]);
        copy_len
    };

    let header = FrameHeader::from_buffer_mut(buf);
    header.width.store(w, Ordering::Release);
    header.height.store(h, Ordering::Release);
    header
        .data_len
        .store(bytes_written as u32, Ordering::Release);

    unsafe { FRAME_COUNT += 1 };
    let fc = unsafe { FRAME_COUNT };
    header.seq.store(fc, Ordering::Release);

    {
        let ctrl = DualControl::as_bytes(full_slice);
        let next_ready = write_idx;
        let next_write = 1 - write_idx;
        ctrl.ready_index.store(next_ready, Ordering::Release);
        ctrl.write_index.store(next_write, Ordering::Release);
    }

    frame_event.0.set();

    if fc <= 3 || fc % 300 == 0 {
        eprintln!(
            "[renderer] shm frame #{}: {}x{} {} bytes (buf={})",
            fc, w, h, bytes_written, write_idx
        );
    }
}

pub fn fps_report_system(
    mut tracker: ResMut<FpsTracker>,
    time: Res<Time>,
    mut ipc: ResMut<crate::ipc::IpcChannel>,
) {
    let now = time.elapsed_secs_f64();
    tracker.frames_since_report += 1;
    tracker.total_frames += 1;

    let elapsed = now - tracker.last_report_time;
    if elapsed >= FPS_REPORT_INTERVAL_SECS && tracker.frames_since_report > 0 {
        let fps = tracker.frames_since_report as f64 / elapsed;
        tracker.last_fps = fps;
        tracker.frames_since_report = 0;
        tracker.last_report_time = now;

        ipc.send(ToDesktop::FpsReport {
            fps,
            frame_count: tracker.total_frames,
        });

        if tracker.total_frames <= 10 || tracker.total_frames % 600 == 0 {
            eprintln!(
                "[renderer] fps={:.1} total_frames={}",
                fps, tracker.total_frames
            );
        }
    }
}

impl Default for FpsTracker {
    fn default() -> Self {
        Self {
            frames_since_report: 0,
            last_report_time: 0.0,
            last_fps: 0.0,
            total_frames: 0,
        }
    }
}
