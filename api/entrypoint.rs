use crate::environ::Environ;

pub fn fiber_entrypoint(environ_cstr: *const i8, main: fn(Environ)) {
    let environ_cstr = unsafe { core::ffi::CStr::from_ptr(environ_cstr) };
    let environ_str = environ_cstr.to_str().expect("environ is not valid string");
    let environ = Environ::from_str(environ_str);
    main(environ);
}
