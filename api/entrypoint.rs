use crate::environ::Environ;

pub extern "C" fn fiber_entrypoint(environ_cstr: *const i8, main: fn(Environ)) {
    let environ_cstr = unsafe { core::ffi::CStr::from_ptr(environ_cstr) };
    let environ_str = environ_cstr.to_str().expect("environ is not valid string");
    let raw_environ = serde_json::from_str(environ_str).expect("environ is not valid JSON");
    let environ = Environ::from_raw(raw_environ);
    main(environ);
}
