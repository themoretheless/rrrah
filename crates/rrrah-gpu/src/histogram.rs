//! One active GPU histogram plus one replaceable input; only 1100 bytes read back.
use rrrah_core::{
    MemoryBudget,
    histogram::{Histogram, MAX_SAMPLES},
};
use std::sync::{Arc, mpsc};
use wgpu::util::DeviceExt;
type Cancel = Arc<dyn Fn() -> bool + Send + Sync>;
const WORDS: usize = 275;
pub const HISTOGRAM_JOB_BYTES: u64 = 32 + 2 * WORDS as u64 * 4;
struct Input {
    buffer: wgpu::Buffer,
    dimensions: [u32; 2],
    crop: rrrah_core::Rect,
    resources: crate::GpuResourceLease,
    cancelled: Cancel,
}
struct Done {
    result: Result<(), String>,
    cancelled: Cancel,
}
struct Readback {
    buffer: wgpu::Buffer,
    _reservation: Arc<rrrah_core::Reservation>,
}
pub struct GpuHistogram {
    revision: Arc<std::sync::atomic::AtomicU64>,
    readback: Option<Readback>,
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
    budget: MemoryBudget,
    sender: mpsc::SyncSender<Done>,
    receiver: mpsc::Receiver<Done>,
    active: bool,
    pending: Option<Input>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl std::fmt::Debug for GpuHistogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuHistogram")
            .field("active", &self.active)
            .field("pending", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}

impl GpuHistogram {
    pub fn new(device: &wgpu::Device, budget: MemoryBudget, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rrrah bounded histogram"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/histogram.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Rrrah bounded histogram"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let (sender, receiver) = mpsc::sync_channel(1);
        Self {
            revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            readback: None,
            device: device.clone(),
            pipeline,
            budget,
            sender,
            receiver,
            active: false,
            pending: None,
            wake,
        }
    }
    /// Calibrated linear sRGB samples only, sampled inside the active crop.
    pub fn submit_bayer(
        &mut self,
        queue: &wgpu::Queue,
        input: &crate::ResidentLinearBayer,
        cancelled: Cancel,
    ) -> Result<(), String> {
        let job = Input {
            buffer: input.buffer().clone(),
            dimensions: input.dimensions(),
            crop: input.crop,
            resources: input.resource_lease(),
            cancelled,
        };
        self.submit(queue, job)
    }
    fn submit(&mut self, queue: &wgpu::Queue, mut input: Input) -> Result<(), String> {
        if (input.cancelled)() {
            return Ok(());
        }
        let id = self.revision.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
        let revision = self.revision.clone();
        let cancelled = input.cancelled;
        input.cancelled =
            Arc::new(move || cancelled() || revision.load(std::sync::atomic::Ordering::Acquire) != id);
        if self.active {
            self.pending = Some(input);
            Ok(())
        } else {
            self.start(queue, input)
        }
    }
    fn start(&mut self, queue: &wgpu::Queue, input: Input) -> Result<(), String> {
        if input.buffer.size() > self.device.limits().max_storage_buffer_binding_size {
            return Err("histogram source exceeds this adapter's storage binding limit".into());
        }
        let crop = input.crop;
        let [width, height] = input.dimensions;
        let valid = crop.x.checked_add(crop.width).is_some_and(|end| end <= width)
            && crop.y.checked_add(crop.height).is_some_and(|end| end <= height)
            && u64::from(width)
                .checked_mul(u64::from(height))
                .and_then(|pixels| pixels.checked_mul(16))
                .is_some_and(|bytes| bytes <= input.buffer.size());
        if !valid {
            return Err("histogram crop/source extent is invalid".into());
        }
        let count = crop
            .width
            .checked_mul(crop.height)
            .ok_or("histogram dimensions overflow")?;
        if count == 0 {
            return Err("empty histogram crop".into());
        }
        let stride = count.div_ceil(MAX_SAMPLES as u32);
        let samples = count.div_ceil(stride);
        let reservation = self
            .budget
            .try_reserve(HISTOGRAM_JOB_BYTES)
            .map_err(|e| e.to_string())?;
        let output = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("histogram counters"),
            contents: &[0; WORDS * 4],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram readback"),
            size: WORDS as u64 * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = [
            input.dimensions[0],
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            stride,
            samples,
            0,
        ];
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("histogram dimensions"),
            contents: bytemuck::cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(samples.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &read, 0, WORDS as u64 * 4);
        queue.submit([encoder.finish()]);
        let reservation = Arc::new(reservation);
        self.readback = Some(Readback {
            buffer: read.clone(),
            _reservation: reservation.clone(),
        });
        let sender = self.sender.clone();
        let wake = self.wake.clone();
        self.active = true;
        // Capture credits, never a Buffer/Device: a callback owning the mapped
        // buffer would keep its own device/context alive after service teardown.
        read.slice(..).map_async(wgpu::MapMode::Read, move |status| {
            drop(input.resources);
            drop(reservation);
            let _ = sender.try_send(Done {
                result: status.map_err(|e| e.to_string()),
                cancelled: input.cancelled,
            });
            wake();
        });
        Ok(())
    }
    /// Call after device polling. Obsolete generations never reach the UI.
    ///
    /// # Panics
    /// Panics if an internal completion arrives without its owned readback.
    pub fn poll(&mut self, queue: &wgpu::Queue) -> Option<Result<Histogram, String>> {
        let done = self.receiver.try_recv().ok()?;
        self.active = false;
        let readback = self.readback.take().expect("active histogram owns readback");
        let result = if (done.cancelled)() {
            None
        } else {
            Some(done.result.and_then(|()| read_histogram(&readback.buffer)))
        };
        readback.buffer.unmap();
        drop(readback);

        if let Some(pending) = self.pending.take()
            && !(pending.cancelled)()
            && let Err(error) = self.start(queue, pending)
        {
            return Some(Err(error));
        }
        result
    }
    pub fn is_pending(&self) -> bool {
        self.active || self.pending.is_some()
    }
    pub fn cancel_pending(&mut self) {
        self.revision.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.pending = None;
    }
}
fn read_histogram(buffer: &wgpu::Buffer) -> Result<Histogram, String> {
    let view = buffer.slice(..).get_mapped_range().map_err(|e| e.to_string())?;
    let words = bytemuck::cast_slice::<u8, u32>(&view);
    let mut histogram = Histogram::default();
    for c in 0..4 {
        for i in 0..64 {
            histogram.channels[c][i] = u64::from(words[c * 64 + i]);
        }
        histogram.negative[c] = u64::from(words[256 + c]);
        histogram.zero[c] = u64::from(words[260 + c]);
        histogram.underflow[c] = u64::from(words[264 + c]);
        histogram.overflow[c] = u64::from(words[268 + c]);
    }
    histogram.sampled = u64::from(words[272]);
    histogram.invalid = u64::from(words[273]);
    histogram.transparent = u64::from(words[274]);
    Ok(histogram)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "macos")]
    fn odd_crops_extreme_values_and_repeated_navigation_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let width = 513u32;
        let height = 259u32;
        let patches = [
            [0.23, 0.51, 1.13, 1.],
            [-1., 0., 4., 1.],
            [f32::MAX, f32::MAX, f32::MAX, 1.],
            [1e-20, 1e-20, 1e-20, 1.],
            [f32::NAN, 1., 1., 1.],
            [1., 1., 1., 0.],
            [1., 1., 1., -0.1],
            [1., 1., 1., 1.1],
            [f32::INFINITY, 0., 0., 1.],
        ];
        let values: Vec<f32> = (0..width * height)
            .flat_map(|i| patches[i as usize % patches.len()])
            .collect();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&values),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let make = |crop| Input {
            buffer: buffer.clone(),
            dimensions: [width, height],
            crop,
            resources: Default::default(),
            cancelled: Arc::new(|| false),
        };
        let budget = MemoryBudget::new(HISTOGRAM_JOB_BYTES);
        let mut worker = GpuHistogram::new(&device, budget.clone(), Arc::new(|| {}));
        for crop in [
            rrrah_core::Rect {
                x: 3,
                y: 5,
                width: 507,
                height: 251,
            },
            rrrah_core::Rect {
                x: 512,
                y: 258,
                width: 1,
                height: 1,
            },
            rrrah_core::Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
        ] {
            let mut cropped = Vec::new();
            for y in crop.y..crop.y + crop.height {
                let start = ((y * width + crop.x) * 4) as usize;
                cropped.extend_from_slice(&values[start..start + crop.width as usize * 4]);
            }
            for _ in 0..12 {
                worker.submit(&queue, make(crop)).unwrap();
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let actual = worker.poll(&queue).unwrap().unwrap();
                assert_eq!(
                    actual,
                    Histogram::linear_rgba(&cropped, MAX_SAMPLES, 0., || false).unwrap()
                );
                assert_eq!(budget.used(), 0);
            }
        }
        for crop in [
            rrrah_core::Rect {
                x: width,
                y: 0,
                width: 1,
                height: 1,
            },
            rrrah_core::Rect {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 1,
            },
            rrrah_core::Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            },
        ] {
            assert!(worker.submit(&queue, make(crop)).is_err());
            assert_eq!(budget.used(), 0);
        }
        let mut short = make(rrrah_core::Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        });
        short.dimensions = [width, height + 1];
        assert!(worker.submit(&queue, short).is_err());
        eprintln!(
            "EXTENDED_HISTOGRAM crops=3 repeats=36 exact_RGBY=true max_samples=65536 invalid_extents=4 credits_final=0"
        );
    }
    use super::*;
    #[test]
    fn histogram_shader_validates_and_storage_is_bounded() {
        let m = wgpu::naga::front::wgsl::parse_str(include_str!("../shaders/histogram.wgsl")).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&m)
        .unwrap();
        assert_eq!(HISTOGRAM_JOB_BYTES, 2232);
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn newer_frame_supersedes_result_even_within_one_decode_generation_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.23f32, 0.51, 1.13, 1.]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let make = || Input {
            buffer: buffer.clone(),
            dimensions: [1, 1],
            crop: rrrah_core::Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            resources: Default::default(),
            cancelled: Arc::new(|| false),
        };
        let budget = MemoryBudget::new(HISTOGRAM_JOB_BYTES);
        let mut h = GpuHistogram::new(&device, budget.clone(), Arc::new(|| {}));
        h.submit(&queue, make()).unwrap();
        h.submit(&queue, make()).unwrap();
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(
            h.poll(&queue).is_none(),
            "older animation/page histogram must not be published"
        );
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(h.poll(&queue).unwrap().unwrap().sampled, 1);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn latest_generation_replaces_pending_and_obsolete_result_is_not_published_metal() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.23f32, 0.51, 1.13, 1.]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let generation = Arc::new(AtomicU64::new(1));
        let make = |expected| {
            let generation = generation.clone();
            Input {
                buffer: buffer.clone(),
                dimensions: [1, 1],
                crop: rrrah_core::Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                resources: Default::default(),
                cancelled: Arc::new(move || generation.load(Ordering::Acquire) != expected),
            }
        };
        let budget = MemoryBudget::new(HISTOGRAM_JOB_BYTES);
        let mut h = GpuHistogram::new(&device, budget.clone(), Arc::new(|| {}));
        h.submit(&queue, make(1)).unwrap();
        generation.store(2, Ordering::Release);
        h.submit(&queue, make(2)).unwrap();
        generation.store(3, Ordering::Release);
        h.submit(&queue, make(3)).unwrap();
        assert!(h.active && h.pending.is_some());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(h.poll(&queue).is_none());
        assert!(h.active && h.pending.is_none());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(h.poll(&queue).unwrap().unwrap().sampled, 1);
        assert!(!h.active);
        assert_eq!(budget.used(), 0);
        eprintln!(
            "GPU histogram navigation: obsolete result absent; middle pending replaced; newest exact; one active plus one pending; final=0"
        );
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn histogram_matches_analytic_cpu_oracle_and_refuses_budget_metal() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&Default::default())).expect("actual Metal required");
        eprintln!("histogram adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let values: Vec<f32> = [
            [0.23, 0.51, 1.13, 1.],
            [-1., 0., 4., 1.],
            [0., 0., 0., 0.],
            [f32::NAN, 0., 0., 1.],
        ]
        .repeat(32768)
        .into_iter()
        .flatten()
        .collect();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&values),
            usage: wgpu::BufferUsages::STORAGE,
        });
        // 131072 pixels -> stride 2, deliberately includes transparent and ordinary pixels.
        let make = || Input {
            buffer: buffer.clone(),
            dimensions: [512, 256],
            crop: rrrah_core::Rect {
                x: 0,
                y: 0,
                width: 512,
                height: 256,
            },
            resources: Default::default(),
            cancelled: Arc::new(|| false),
        };
        let budget = MemoryBudget::new(HISTOGRAM_JOB_BYTES);
        let mut worker = GpuHistogram::new(&device, budget.clone(), Arc::new(|| {}));
        worker.submit(&queue, make()).unwrap();
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let histogram = worker.poll(&queue).unwrap().unwrap();
        let expected = Histogram::linear_rgba(&values, MAX_SAMPLES, 0., || false).unwrap();
        assert_eq!(histogram, expected);
        assert_eq!(histogram.sampled, 65536);
        assert_eq!(budget.used(), 0);
        let mut refused = GpuHistogram::new(
            &device,
            MemoryBudget::new(HISTOGRAM_JOB_BYTES - 1),
            Arc::new(|| {}),
        );
        assert!(refused.submit(&queue, make()).is_err());
        // Small input exercises negative, invalid and transparent policy without subsampling.
        let mut small = make();
        small.crop.width = 4;
        small.crop.height = 1;
        worker.submit(&queue, small).unwrap();
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let h = worker.poll(&queue).unwrap().unwrap();
        assert_eq!(
            h,
            Histogram::linear_rgba(&values[..16], MAX_SAMPLES, 0., || false).unwrap()
        );
        let mut dropped = GpuHistogram::new(&device, budget.clone(), Arc::new(|| {}));
        dropped.submit(&queue, make()).unwrap();
        assert_eq!(budget.used(), HISTOGRAM_JOB_BYTES);
        drop(dropped);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
        eprintln!(
            "GPU histogram: CPU analytic RGB/Y exact; 65536 samples; signed/invalid/alpha exact; 1100-byte readback; budget refusal; final=0"
        );
    }
}
