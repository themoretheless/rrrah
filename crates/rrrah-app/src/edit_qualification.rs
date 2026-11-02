//! Explicit opt-in, bounded live-window frame qualification for editor work.
//! Queue completion is not physical display/scanout latency.
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Instant;
struct Probe {
    path: std::path::PathBuf,
    dispatched: AtomicUsize,
    samples: Mutex<Vec<(f64, f64)>>,
    native_peak: AtomicU64,
}
static PROBE: OnceLock<Option<Arc<Probe>>> = OnceLock::new();
pub fn frame(
    queue: &wgpu::Queue,
    device: &wgpu::Device,
    window: &Arc<winit::window::Window>,
    started: Instant,
    cpu_ms: f64,
    cpu_peak: Option<u64>,
    gpu_peaks: [Option<u64>; 3],
) {
    let Some(probe) = PROBE.get_or_init(|| {
        std::env::var_os("RRRAH_EDIT_FRAME_TRACE").map(|path| {
            Arc::new(Probe {
                path: path.into(),
                dispatched: AtomicUsize::new(0),
                native_peak: AtomicU64::new(0),
                samples: Mutex::new(Vec::with_capacity(64)),
            })
        })
    }) else {
        return;
    };
    if probe.dispatched.fetch_add(1, Ordering::Relaxed) >= 64 {
        return;
    }
    let probe = probe.clone();
    let native = native_counter(device);
    if let Ok((_, bytes)) = &native {
        probe.native_peak.fetch_max(*bytes, Ordering::Relaxed);
    }
    let native_peak = probe.native_peak.load(Ordering::Relaxed);
    let allocator = format!("{:?}", device.generate_allocator_report());
    queue.on_submitted_work_done(move|| {
        let mut samples=probe.samples.lock().unwrap();samples.push((cpu_ms,started.elapsed().as_secs_f64()*1000.));
        if samples.len()==64 {
            let raw=samples.clone();let mut cpu:Vec<_>=samples.iter().skip(3).map(|v|v.0).collect();let mut complete:Vec<_>=samples.iter().skip(3).map(|v|v.1).collect();cpu.sort_by(f64::total_cmp);complete.sort_by(f64::total_cmp);
            let report=serde_json::json!({"scope":"live Metal window; CPU frame preparation/submission and host-observed GPU queue completion, not physical presentation", "warmups":3,"samples":raw,"median_cpu_ms":cpu[30],"p95_cpu_ms":cpu[57],"median_queue_complete_ms":complete[30],"p95_queue_complete_ms":complete[57],"logical_cpu_peak_bytes":cpu_peak,"logical_gpu_peaks_raw_raster_strip_bytes":gpu_peaks,"allocator_report":allocator,"native_metal_counter":native,"native_metal_sampled_peak_bytes":native_peak});
            let path=probe.path.clone();std::thread::spawn(move||{if let Err(error)=std::fs::write(path,serde_json::to_vec_pretty(&report).unwrap()) {log::error!("edit frame qualification write failed: {error}");}});
        }
    });
    window.request_redraw();
}

static ADAPTER: OnceLock<wgpu::AdapterInfo> = OnceLock::new();
pub fn register_adapter(info: &wgpu::AdapterInfo) {
    let _ = ADAPTER.set(info.clone());
}
#[cfg(target_os = "macos")]
fn native_counter(device: &wgpu::Device) -> Result<(u64, u64), String> {
    use objc2_metal::{MTLCopyAllDevices, MTLDevice};
    static VALIDATED: OnceLock<Result<u64, String>> = OnceLock::new();
    let id = VALIDATED
        .get_or_init(|| {
            let info = ADAPTER.get().ok_or("missing selected adapter")?;
            if info.backend != wgpu::Backend::Metal {
                return Err("not Metal".into());
            }
            let all = MTLCopyAllDevices();
            if all.len() != 1 {
                return Err("native counter requires a single unambiguous device".into());
            }
            let native = all.objectAtIndex(0);
            if native.name().to_string() != info.name {
                return Err("native/selected adapter mismatch".into());
            }
            let before = native.currentAllocatedSize() as u64;
            const BYTES: u64 = 8 * 1024 * 1024;
            let challenge = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("qualification allocation challenge"),
                size: BYTES,
                usage: wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            });
            challenge
                .get_mapped_range_mut(..)
                .map_err(|e| e.to_string())?
                .slice(..)
                .fill(17);
            challenge.unmap();
            let during = native.currentAllocatedSize() as u64;
            drop(challenge);
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            let after = native.currentAllocatedSize() as u64;
            if during < before + BYTES || after >= during {
                return Err(format!(
                    "native counter failed actual wgpu allocation challenge: {before}/{during}/{after}"
                ));
            }
            Ok(native.registryID())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let all = MTLCopyAllDevices();
    let native = all
        .iter()
        .find(|d| d.registryID() == *id)
        .ok_or("validated Metal device disappeared")?;
    Ok((*id, native.currentAllocatedSize() as u64))
}
#[cfg(not(target_os = "macos"))]
fn native_counter(_: &wgpu::Device) -> Result<(u64, u64), String> {
    Err("native Metal counter unavailable".into())
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn native_counter_tracks_selected_wgpu_allocations_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        register_adapter(&adapter.get_info());
        let (device, _) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let (id, before) = native_counter(&device).unwrap();
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16 * 1024 * 1024,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        b.get_mapped_range_mut(..).unwrap().slice(..).fill(5);
        b.unmap();
        let (_, during) = native_counter(&device).unwrap();
        assert!(during >= before + 16 * 1024 * 1024);
        drop(b);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let (_, after) = native_counter(&device).unwrap();
        assert!(after < during);
        eprintln!("NATIVE_METAL_COUNTER registry={id} before={before} during={during} after={after}");
    }
}

/// Arm comparison-only traces explicitly; normal builds and frames are unaffected.
pub fn capture_comparison_frames() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RRRAH_COMPARE_FRAME_TRACE").is_some())
}
