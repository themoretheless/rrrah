//! CUDA FFI boundary. Function pointers remain valid through the owned library;
//! contexts and device allocations never escape the creating thread or call.
use crate::CudaError;
use libloading::Library;
use rrrah_memory::Reservation;
use std::{
    ffi::{CString, c_char, c_void},
    sync::Arc,
};
type Handle = *mut c_void;
type Init = unsafe extern "system" fn(u32) -> i32;
type DeviceGet = unsafe extern "system" fn(*mut i32, i32) -> i32;
type ContextCreate = unsafe extern "system" fn(*mut Handle, u32, i32) -> i32;
type ContextDestroy = unsafe extern "system" fn(Handle) -> i32;
type Synchronize = unsafe extern "system" fn() -> i32;
type ModuleLoad = unsafe extern "system" fn(*mut Handle, *const c_void) -> i32;
type FunctionGet = unsafe extern "system" fn(*mut Handle, Handle, *const c_char) -> i32;
type Allocate = unsafe extern "system" fn(*mut u64, usize) -> i32;
type CopyToDevice = unsafe extern "system" fn(u64, *const c_void, usize) -> i32;
type CopyToHost = unsafe extern "system" fn(*mut c_void, u64, usize) -> i32;
type Launch = unsafe extern "system" fn(
    Handle,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    Handle,
    *mut *mut c_void,
    *mut *mut c_void,
) -> i32;
#[derive(Debug)]
pub(crate) struct Driver {
    _library: Library,
    device_get: DeviceGet,
    context_create: ContextCreate,
    context_destroy: ContextDestroy,
    synchronize: Synchronize,
    module_load: ModuleLoad,
    function_get: FunctionGet,
    allocate: Allocate,
    copy_to_device: CopyToDevice,
    copy_to_host: CopyToHost,
    launch: Launch,
}
fn check(operation: &'static str, status: i32) -> Result<(), CudaError> {
    if status == 0 {
        Ok(())
    } else {
        Err(CudaError::Operation { operation, status })
    }
}
fn library_name() -> Result<&'static str, CudaError> {
    #[cfg(all(target_pointer_width = "64", target_os = "linux"))]
    {
        return Ok("libcuda.so.1");
    }
    #[cfg(all(target_pointer_width = "64", target_os = "windows"))]
    {
        return Ok("nvcuda.dll");
    }
    #[cfg(not(all(target_pointer_width = "64", any(target_os = "linux", target_os = "windows"))))]
    {
        Err(CudaError::UnsupportedPlatform)
    }
}
impl Driver {
    pub(crate) fn open() -> Result<Arc<Self>, CudaError> {
        // SAFETY: explicit system CUDA driver only; all loaded signatures match
        // the 64-bit CUDA Driver API. Library ownership outlives every pointer.
        let library =
            unsafe { Library::new(library_name()?) }.map_err(|error| CudaError::Driver(error.to_string()))?;
        macro_rules! load {
            ($ty:ty, $name:literal) => {
                unsafe {
                    *library
                        .get::<$ty>(concat!($name, "\0").as_bytes())
                        .map_err(|error| CudaError::Driver(error.to_string()))?
                }
            };
        }
        let init = load!(Init, "cuInit");
        // SAFETY: zero is the only supported initialization flag.
        check("cuInit", unsafe { init(0) })?;
        Ok(Arc::new(Self {
            device_get: load!(DeviceGet, "cuDeviceGet"),
            context_create: load!(ContextCreate, "cuCtxCreate_v2"),
            context_destroy: load!(ContextDestroy, "cuCtxDestroy_v2"),
            synchronize: load!(Synchronize, "cuCtxSynchronize"),
            module_load: load!(ModuleLoad, "cuModuleLoadData"),
            function_get: load!(FunctionGet, "cuModuleGetFunction"),
            allocate: load!(Allocate, "cuMemAlloc_v2"),
            copy_to_device: load!(CopyToDevice, "cuMemcpyHtoD_v2"),
            copy_to_host: load!(CopyToHost, "cuMemcpyDtoH_v2"),
            launch: load!(Launch, "cuLaunchKernel"),
            _library: library,
        }))
    }
    pub(crate) fn expose(
        self: &Arc<Self>,
        ordinal: i32,
        pixels: &[[f32; 4]],
        gain: f32,
        output: &mut [[f32; 4]],
        reservation: Reservation,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<(), CudaError> {
        let mut device = 0;
        let mut handle = std::ptr::null_mut();
        // SAFETY: writable stack arguments; ordinal is checked by the driver.
        check("cuDeviceGet", unsafe {
            (self.device_get)(&raw mut device, ordinal)
        })?;
        check("cuCtxCreate_v2", unsafe {
            (self.context_create)(&raw mut handle, 0, device)
        })?;
        let mut context = Context {
            handle,
            driver: self.clone(),
            reservation: Some(reservation),
            _thread: std::marker::PhantomData,
        };
        let ptx = CString::new(include_str!("exposure.ptx")).expect("owned PTX contains no NUL");
        let mut module = std::ptr::null_mut();
        let mut function = std::ptr::null_mut();
        let bytes = std::mem::size_of_val(pixels);
        let mut input_device = 0;
        let mut output_device = 0;
        // SAFETY: null-terminated owned PTX and entry name, valid current context,
        // and writable handles. Context destruction frees modules/allocations.
        check("cuModuleLoadData", unsafe {
            (self.module_load)(&raw mut module, ptx.as_ptr().cast())
        })?;
        check("cuModuleGetFunction", unsafe {
            (self.function_get)(&raw mut function, module, c"rrrah_exposure".as_ptr())
        })?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        check("cuMemAlloc input", unsafe {
            (self.allocate)(&raw mut input_device, bytes)
        })?;
        check("cuMemAlloc output", unsafe {
            (self.allocate)(&raw mut output_device, bytes)
        })?;
        crate::readback::copy_input_chunks(pixels, input_device, cancelled, |chunk, address| {
            // SAFETY: checked contiguous subrange of the allocated device
            // payload and live borrowed CPU slice; no staging copy.
            check("cuMemcpyHtoD_v2", unsafe {
                (self.copy_to_device)(address, chunk.as_ptr().cast(), std::mem::size_of_val(chunk))
            })
        })?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        let mut count = u32::try_from(pixels.len()).map_err(|_| CudaError::Invalid("too many pixels"))?;
        let mut gain = gain;
        let mut parameters = [
            (&raw mut input_device).cast::<c_void>(),
            (&raw mut output_device).cast::<c_void>(),
            (&raw mut count).cast::<c_void>(),
            (&raw mut gain).cast::<c_void>(),
        ];
        // SAFETY: PTX argument types match these four live stack arguments.
        // One thread writes one pixel; the kernel guards the final partial block.
        check("cuLaunchKernel", unsafe {
            (self.launch)(
                function,
                count.div_ceil(256),
                1,
                1,
                256,
                1,
                1,
                0,
                std::ptr::null_mut(),
                parameters.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        })?;
        check("cuCtxSynchronize", unsafe { (self.synchronize)() })?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        // SAFETY: output slice is exactly the allocated device payload length.
        debug_assert_eq!(std::mem::size_of_val(output), bytes);
        crate::readback::copy_chunks(output, output_device, cancelled, |chunk, address| {
            // SAFETY: checked contiguous subrange of the synchronized device
            // allocation and equally sized writable CPU slice.
            check("cuMemcpyDtoH_v2", unsafe {
                (self.copy_to_host)(chunk.as_mut_ptr().cast(), address, std::mem::size_of_val(chunk))
            })
        })?;
        context.close()
    }
}
struct Context {
    handle: Handle,
    driver: Arc<Driver>,
    reservation: Option<Reservation>,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Context {
    fn close(&mut self) -> Result<(), CudaError> {
        // SAFETY: owned context on its creating thread; successful destruction
        // releases all modules/device allocations, including unfinished work.
        check("cuCtxDestroy_v2", unsafe {
            (self.driver.context_destroy)(self.handle)
        })?;
        self.handle = std::ptr::null_mut();
        self.reservation.take();
        Ok(())
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.close().is_err() {
            // Device cleanup is unproven after a driver failure. Conservatively
            // retain accounting and the library rather than freeing live credit.
            if let Some(reservation) = self.reservation.take() {
                std::mem::forget(reservation);
            }
            std::mem::forget(self.driver.clone());
        }
    }
}
