//! Real MEF sensor transport only; authored channels are not camera color.
#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned RRRAH_MEF_SOURCE and actual Metal; roughly 2GB tracked buffers"]
fn original_mef_all_sensor_codes_survive_managed_metal_compute() {
    let path = std::env::var_os("RRRAH_MEF_SOURCE").expect("pinned MEF source required");
    let hash = std::process::Command::new("shasum").args(["-a", "256"]).arg(&path).output().unwrap();
    assert!(hash.status.success());
    assert!(String::from_utf8(hash.stdout).unwrap().starts_with(
        "bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c "));
    let count = 4016usize * 5344;
    let bytes = count as u64 * 16;
    let sensor_bytes = count as u64 * 2;
    let cpu = rrrah_core::MemoryBudget::new(bytes * 2 + sensor_bytes);
    let sensor = rrrah_decode::decode_mef_zd_sensor_file(
        &rrrah_decode::DecodeRequest::new(path), &cpu).unwrap();
    assert_eq!(sensor.len(), count);
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance, &wgpu::RequestAdapterOptions::default())).expect("actual GPU required");
    assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
    eprintln!("MEF sensor compute adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits { max_buffer_size: bytes, ..Default::default() },
        ..Default::default()
    })).unwrap();
    let limits = device.limits();
    let alignment = u64::from(limits.min_storage_buffer_offset_alignment).max(16) / 16;
    let maximum = (u64::from(limits.max_storage_buffer_binding_size) / 16)
        .min(u64::from(limits.max_compute_workgroups_per_dimension) * 64);
    let chunk = maximum / alignment * alignment;
    let gpu_bytes = bytes * 3 + (count as u64).div_ceil(chunk) * 16;
    let gpu = rrrah_core::MemoryBudget::new(gpu_bytes);
    let mut input = cpu.try_buffer(count, [0f32; 4]).unwrap();
    for (i, (pixel, &code)) in input.iter_mut().zip(sensor.iter()).enumerate() {
        let value = f32::from(code);
        *pixel = [value / 4096., -value / 8192., value / 2048., (i % 4) as f32 / 4.];
    }
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let started = std::time::Instant::now();
    let output = compute.execute_managed(&queue, &input, 1., &gpu, &cpu).unwrap();
    let elapsed = started.elapsed().as_secs_f64() * 1000.;
    for (i, (pixel, &code)) in output.iter().zip(sensor.iter()).enumerate() {
        let value = f32::from(code);
        let expected = [value / 2048., -value / 4096., value / 1024., (i % 4) as f32 / 4.];
        assert_eq!(*pixel, expected, "sensor position {i}");
    }
    assert_eq!(gpu.peak(), gpu_bytes);
    assert_eq!(gpu.used(), 0);
    assert_eq!(cpu.peak(), bytes * 2 + sensor_bytes);
    drop(output);
    drop(input);
    drop(sensor);
    assert_eq!(cpu.used(), 0);
    eprintln!("MEF full sensor compute: samples={count}; all authored channels exact; end_to_end_ms={elapsed:.3}; cpu_peak={}; gpu_peak={}; final=0; camera_WB_color_unqualified", cpu.peak(), gpu.peak());
}
