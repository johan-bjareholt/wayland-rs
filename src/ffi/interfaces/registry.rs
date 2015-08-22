use std::ptr;

use libc::{c_int, c_void, c_char, uint32_t};

use ffi::abi::{wl_proxy, wl_interface};
#[cfg(not(feature = "dlopen"))]
use ffi::abi::{wl_proxy_destroy, wl_proxy_add_listener, wl_proxy_set_user_data,
               wl_proxy_get_user_data, wl_proxy_marshal_constructor};
#[cfg(feature = "dlopen")]
use ffi::abi::WAYLAND_CLIENT_HANDLE as WCH;

pub enum wl_registry { }

#[repr(C)]
pub struct wl_registry_listener {
    pub global: extern fn(data: *mut c_void,
                          registry: *mut wl_registry,
                          name: uint32_t,
                          interface: *const c_char,
                          version: uint32_t
                         ),
    pub global_remove: extern fn(data: *mut c_void,
                                 registry: *mut wl_registry,
                                 name: uint32_t
                                )
}

const WL_REGISTRY_BIND: uint32_t = 0;

#[inline(always)]
pub unsafe fn wl_registry_add_listener(registry: *mut wl_registry,
                                       listener: *const wl_registry_listener,
                                       data: *mut c_void
                                      ) -> c_int {
    ffi_dispatch!(WCH, wl_proxy_add_listener,
        registry as *mut wl_proxy,
        listener as *mut extern fn(),
        data
    )
}

#[inline(always)]
pub unsafe fn wl_registry_set_user_data(registry: *mut wl_registry, data: *mut c_void) {
    ffi_dispatch!(WCH, wl_proxy_set_user_data,registry as *mut wl_proxy, data)
}

#[inline(always)]
pub unsafe fn wl_registry_get_user_data(registry: *mut wl_registry) -> *mut c_void {
    ffi_dispatch!(WCH, wl_proxy_get_user_data,registry as *mut wl_proxy)
}

#[inline(always)]
pub unsafe fn wl_registry_destroy(registry: *mut wl_registry) {
    ffi_dispatch!(WCH, wl_proxy_destroy,registry as *mut wl_proxy)
}

#[inline(always)]
pub unsafe fn wl_registry_bind(registry: *mut wl_registry,
                               name: uint32_t,
                               interface: *const wl_interface,
                               version: uint32_t
                              ) -> *mut c_void {
    let id = ffi_dispatch!(WCH, wl_proxy_marshal_constructor,
        registry as *mut wl_proxy,
        WL_REGISTRY_BIND,
        interface,
        name,
        (*interface).name,
        version,
        ptr::null_mut::<c_void>()
    );
    id as *mut c_void
}