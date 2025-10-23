// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Contains FFI bindings for the wdi library.
//! 
//! // lib.rs or ffi.rs

#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_ushort, c_uchar};

pub const WDI_MAX_STRLEN: usize = 200;

// Windows types
type BOOL = c_int;
type HWND = *mut std::ffi::c_void;
type UINT32 = u32;
type UINT64 = u64;
type DWORD = u32;

#[repr(C)]
pub enum WdiDriverType {
    WinUsb = 0,
    Libusb0,
    LibusbK,
    Cdc,
    User,
    NbDrivers,
}

#[repr(C)]
pub enum WdiLogLevel {
    Debug = 0,
    Info,
    Warning,
    Error,
    None,
}

#[repr(C)]
pub enum WdiError {
    Success = 0,
    ErrorIo = -1,
    ErrorInvalidParam = -2,
    ErrorAccess = -3,
    ErrorNoDevice = -4,
    ErrorNotFound = -5,
    ErrorBusy = -6,
    ErrorTimeout = -7,
    ErrorOverflow = -8,
    ErrorPendingInstallation = -9,
    ErrorInterrupted = -10,
    ErrorResource = -11,
    ErrorNotSupported = -12,
    ErrorExists = -13,
    ErrorUserCancel = -14,
    ErrorNeedsAdmin = -15,
    ErrorWow64 = -16,
    ErrorInfSyntax = -17,
    ErrorCatMissing = -18,
    ErrorUnsigned = -19,
    ErrorOther = -99,
}

#[repr(C)]
pub struct WdiDeviceInfo {
    pub next: *mut WdiDeviceInfo,
    pub vid: c_ushort,
    pub pid: c_ushort,
    pub is_composite: BOOL,
    pub mi: c_uchar,
    pub desc: *mut c_char,
    pub driver: *mut c_char,
    pub device_id: *mut c_char,
    pub hardware_id: *mut c_char,
    pub compatible_id: *mut c_char,
    pub upper_filter: *mut c_char,
    pub driver_version: UINT64,
}

#[repr(C)]
pub struct WdiOptionsCreateList {
    pub list_all: BOOL,
    pub list_hubs: BOOL,
    pub trim_whitespaces: BOOL,
}

#[repr(C)]
pub struct WdiOptionsPrepareDriver {
    pub driver_type: c_int,
    pub vendor_name: *mut c_char,
    pub device_guid: *mut c_char,
    pub disable_cat: BOOL,
    pub disable_signing: BOOL,
    pub cert_subject: *mut c_char,
    pub use_wcid_driver: BOOL,
    pub external_inf: BOOL,
}

#[repr(C)]
pub struct WdiOptionsInstallDriver {
    pub hwnd: HWND,
    pub install_filter_driver: BOOL,
    pub pending_install_timeout: UINT32,
}

#[repr(C)]
pub struct WdiOptionsInstallCert {
    pub hwnd: HWND,
    pub disable_warning: BOOL,
}

#[repr(C)]
pub struct VsFixedFileInfo {
    // Add fields if you need wdi_is_driver_supported
    _unused: [u8; 0],
}

#[link(name = "libwdi", kind = "static")]
unsafe extern "system" {
    pub fn wdi_strerror(errcode: c_int) -> *const c_char;
    
    pub fn wdi_is_driver_supported(
        driver_type: c_int,
        driver_info: *mut VsFixedFileInfo,
    ) -> BOOL;
    
    pub fn wdi_is_file_embedded(path: *const c_char, name: *const c_char) -> BOOL;
    
    pub fn wdi_get_vendor_name(vid: c_ushort) -> *const c_char;
    
    pub fn wdi_create_list(
        list: *mut *mut WdiDeviceInfo,
        options: *mut WdiOptionsCreateList,
    ) -> c_int;
    
    pub fn wdi_destroy_list(list: *mut WdiDeviceInfo) -> c_int;
    
    pub fn wdi_prepare_driver(
        device_info: *mut WdiDeviceInfo,
        path: *const c_char,
        inf_name: *const c_char,
        options: *mut WdiOptionsPrepareDriver,
    ) -> c_int;
    
    pub fn wdi_install_driver(
        device_info: *mut WdiDeviceInfo,
        path: *const c_char,
        inf_name: *const c_char,
        options: *mut WdiOptionsInstallDriver,
    ) -> c_int;
    
    pub fn wdi_install_trusted_certificate(
        cert_name: *const c_char,
        options: *mut WdiOptionsInstallCert,
    ) -> c_int;
    
    pub fn wdi_set_log_level(level: c_int) -> c_int;
    
    pub fn wdi_register_logger(hwnd: HWND, message: u32, buffsize: DWORD) -> c_int;
    
    pub fn wdi_unregister_logger(hwnd: HWND) -> c_int;
    
    pub fn wdi_read_logger(
        buffer: *mut c_char,
        buffer_size: DWORD,
        message_size: *mut DWORD,
    ) -> c_int;
    
    pub fn wdi_get_wdf_version() -> c_int;
}