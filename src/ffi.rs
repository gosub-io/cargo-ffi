use std::ffi::CStr;
use std::os::raw::c_char;
use crate::engine::Engine;

#[repr(C)]
pub struct GosubEngineHandle(*mut Engine);

#[no_mangle]
pub extern "C" fn gosub_engine_new() -> GosubEngineHandle {
    let engine = Box::new(Engine::new());
    GosubEngineHandle(Box::into_raw(engine))
}

#[no_mangle]
pub extern "C" fn gosub_load_url(handle: GosubEngineHandle, url: *const c_char) {
    let engine = unsafe { &mut *handle.0 };
    let url_str = unsafe { CStr::from_ptr(url).to_str().unwrap() };
    engine.load_url(url_str);
}

#[no_mangle]
pub extern "C" fn gosub_tick(handle: GosubEngineHandle) -> bool {
    let engine = unsafe { &mut *handle.0 };
    engine.tick()
}

#[no_mangle]
pub extern "C" fn gosub_render(handle: GosubEngineHandle, output: *mut u8, output_size: usize) -> usize {
    let engine = unsafe { &*handle.0 };
    let rendered_data = engine.render();

    if output_size < rendered_data.len() {
        return 0; // Not enough space in output buffer
    }

    unsafe {
        std::ptr::copy_nonoverlapping(rendered_data.as_ptr(), output, rendered_data.len());
    }

    rendered_data.len()
}


#[no_mangle]
pub extern "C" fn gosub_engine_free(handle: GosubEngineHandle) {
    if !handle.0.is_null() {
        unsafe {
            let _ = Box::from_raw(handle.0);
        }
    }
}