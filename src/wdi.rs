// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Exposes a safe Rust API around libwdi's APIs

use crate::ffi::{WdiDeviceInfo, WdiLogLevel, WdiOptionsCreateList, WdiOptionsPrepareDriver, WdiOptionsInstallDriver};
use crate::ffi::{wdi_create_list, wdi_destroy_list, wdi_prepare_driver, wdi_install_driver, wdi_set_log_level};
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_int;
use std::ptr;

/// Log level for libwdi logging.  Note that libwdi is quite chatty, so the levels are shifted
/// down by one when mapping the standard Rust log levels.
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    None,
}

impl From<log::LevelFilter> for LogLevel {
    fn from(level: log::LevelFilter) -> Self {
        // Shift off by 1 - libwdi is a bit chatty
        match level {
            log::LevelFilter::Trace => LogLevel::Debug,
            log::LevelFilter::Debug => LogLevel::Info,
            log::LevelFilter::Info => LogLevel::Warning,
            log::LevelFilter::Warn => LogLevel::Error,
            log::LevelFilter::Error => LogLevel::Error,
            log::LevelFilter::Off => LogLevel::None,
        }
    }
}

impl From<log::Level> for LogLevel {
    fn from(level: log::Level) -> Self {
        // Shift off by 1 - libwdi is a bit chatty
        match level {
            log::Level::Trace => LogLevel::Debug,
            log::Level::Debug => LogLevel::Info,
            log::Level::Info => LogLevel::Warning,
            log::Level::Warn => LogLevel::Error,
            log::Level::Error => LogLevel::Error,
        }
    }
}

impl From<LogLevel> for c_int {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
            LogLevel::None => 4,
        }
    }
}

impl From<WdiLogLevel> for LogLevel {
    fn from(level: WdiLogLevel) -> Self {
        match level {
            WdiLogLevel::Debug => LogLevel::Debug,
            WdiLogLevel::Info => LogLevel::Info,
            WdiLogLevel::Warning => LogLevel::Warning,
            WdiLogLevel::Error => LogLevel::Error,
            WdiLogLevel::None => LogLevel::None,
        }
    }
}

/// Error codes returned by libwdi
#[derive(Debug)]
pub enum Error {
    Io,
    InvalidParam,
    Access,
    NoDevice,
    NotFound,
    Busy,
    Timeout,
    Overflow,
    PendingInstallation,
    Interrupted,
    Resource,
    NotSupported,
    Exists,
    UserCancel,
    NeedsAdmin,
    Wow64,
    InfSyntax,
    CatMissing,
    Unsigned,
    Other,
    Unknown(c_int),
}

impl Error {
    fn from_code(code: c_int) -> Result<(), Self> {
        match code {
            0 => Ok(()),
            -1 => Err(Error::Io),
            -2 => Err(Error::InvalidParam),
            -3 => Err(Error::Access),
            -4 => Err(Error::NoDevice),
            -5 => Err(Error::NotFound),
            -6 => Err(Error::Busy),
            -7 => Err(Error::Timeout),
            -8 => Err(Error::Overflow),
            -9 => Err(Error::PendingInstallation),
            -10 => Err(Error::Interrupted),
            -11 => Err(Error::Resource),
            -12 => Err(Error::NotSupported),
            -13 => Err(Error::Exists),
            -14 => Err(Error::UserCancel),
            -15 => Err(Error::NeedsAdmin),
            -16 => Err(Error::Wow64),
            -17 => Err(Error::InfSyntax),
            -18 => Err(Error::CatMissing),
            -19 => Err(Error::Unsigned),
            -99 => Err(Error::Other),
            code => Err(Error::Unknown(code)),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

/// Driver types supported by libwdi
#[derive(Debug, Clone, Copy)]
pub enum DriverType {
    WinUsb,
    LibUsb0,
    LibUsbK,
    Cdc,
    User,
}

impl DriverType {
    fn to_c_int(self) -> c_int {
        match self {
            DriverType::WinUsb => 0,
            DriverType::LibUsb0 => 1,
            DriverType::LibUsbK => 2,
            DriverType::Cdc => 3,
            DriverType::User => 4,
        }
    }
}

/// Represents a connected device.  The fields correspond to those returned by libwdi
#[derive(Debug, Clone)]
pub struct Device {
    pub vid: u16,
    pub pid: u16,
    pub is_composite: bool,
    pub mi: u8,
    pub desc: Option<String>,
    pub driver: Option<String>,
    pub device_id: Option<String>,
    pub hardware_id: Option<String>,
    pub compatible_id: Option<String>,
    pub upper_filter: Option<String>,
    pub driver_version: u64,
}

impl Device {
    unsafe fn from_raw(raw: *const WdiDeviceInfo) -> Self {
        let raw = unsafe { &*raw };
        Device {
            vid: raw.vid,
            pid: raw.pid,
            is_composite: raw.is_composite != 0,
            mi: raw.mi,
            desc: unsafe{ ptr_to_string(raw.desc) },
            driver: unsafe{ ptr_to_string(raw.driver) },
            device_id: unsafe{ ptr_to_string(raw.device_id) },
            hardware_id: unsafe{ ptr_to_string(raw.hardware_id) },
            compatible_id: unsafe{ ptr_to_string(raw.compatible_id) },
            upper_filter: unsafe{ ptr_to_string(raw.upper_filter) },
            driver_version: raw.driver_version,
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{:04X}:{:04X} {}",
            self.vid,
            self.pid,
            self.desc.as_deref().unwrap_or("(no description)")
        )
    }
}

unsafe fn ptr_to_string(ptr: *mut i8) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_owned()) }
    }
}

/// Represents a list of connected devices
/// 
/// Use the [`iter`](DeviceList::iter) method to iterate over the devices
#[derive(Debug)]
pub struct DeviceList {
    head: *mut WdiDeviceInfo,
}

impl DeviceList {
    /// Returns an iterator over the devices in the list
    pub fn iter(&self) -> DeviceIter {
        DeviceIter {
            current: self.head,
        }
    }

    /// Gets the number of devices in the list
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Checks if the device list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Gets the device at the specified index, if it exists
    pub fn get(&self, index: usize) -> Option<Device> {
        self.iter().nth(index)
    }

    /// Filters the device list by VID and PID, returning a vector of matching [`Device`]s
    pub fn from_vid_pid(&self, vid: u16, pid: u16) -> Vec<Device> {
        self.iter()
            .filter(|d| d.vid == vid && d.pid == pid)
            .collect()
    }
}

impl Drop for DeviceList {
    fn drop(&mut self) {
        if !self.head.is_null() {
            unsafe {
                wdi_destroy_list(self.head);
            }
        }
    }
}

/// Iterator over the devices in a [`DeviceList`]
pub struct DeviceIter {
    current: *mut WdiDeviceInfo,
}

impl Iterator for DeviceIter {
    type Item = Device;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            None
        } else {
            unsafe {
                let device = Device::from_raw(self.current);
                self.current = (*self.current).next;
                Some(device)
            }
        }
    }
}

/// Options for creating a device list, as exposed by libwdi
#[derive(Debug, Clone)]
pub struct CreateListOptions {
    pub list_all: bool,
    pub list_hubs: bool,
    pub trim_whitespaces: bool,
}

impl Default for CreateListOptions {
    fn default() -> Self {
        CreateListOptions {
            list_all: false,
            list_hubs: false,
            trim_whitespaces: true,
        }
    }
}

/// Enumerates connected devices and returns a [`DeviceList`]
/// 
/// # Arguments
/// * `options` - The options to use when creating the device list.
pub fn create_list(options: CreateListOptions) -> Result<DeviceList, Error> {
    let mut opts = WdiOptionsCreateList {
        list_all: options.list_all as c_int,
        list_hubs: options.list_hubs as c_int,
        trim_whitespaces: options.trim_whitespaces as c_int,
    };

    let mut list: *mut WdiDeviceInfo = ptr::null_mut();
    
    unsafe {
        let result = wdi_create_list(&mut list, &mut opts);
        Error::from_code(result)?;
    }

    Ok(DeviceList { head: list })
}

/// Options for preparing a driver, as exposed by libwdi
/// 
/// You can use `default()` to construct.
#[derive(Debug, Clone)]
pub struct PrepareDriverOptions {
    pub driver_type: DriverType,
    pub vendor_name: Option<String>,
    pub device_guid: Option<String>,
    pub disable_cat: bool,
    pub disable_signing: bool,
    pub cert_subject: Option<String>,
    pub use_wcid_driver: bool,

    /// Indicates whether to use an external INF file.  Note that `wdi-rs` overrides this
    /// when using a custom INF file via the higher level APIs.
    pub external_inf: bool,
}

impl Default for PrepareDriverOptions {
    fn default() -> Self {
        PrepareDriverOptions {
            driver_type: DriverType::WinUsb,
            vendor_name: None,
            device_guid: None,
            disable_cat: false,
            disable_signing: false,
            cert_subject: None,
            use_wcid_driver: false,
            external_inf: false,
        }
    }
}

/// Prepares a driver for installation using libwdi
/// 
/// # Arguments
/// * `device` - The device for which to prepare the driver.
/// * `path` - The path where the driver files will be created.
/// * `inf_name` - The name of the INF file to create, or use, if using an existing one.
/// * `options` - The options to use when preparing the driver.
/// 
/// # Errors
/// * Returns an `Error` if the preparation fails.
pub fn prepare_driver(
    device: &Device,
    path: &str,
    inf_name: &str,
    options: &PrepareDriverOptions,
) -> Result<(), Error> {
    let path_c = CString::new(path).map_err(|_| Error::InvalidParam)?;
    let inf_name_c = CString::new(inf_name).map_err(|_| Error::InvalidParam)?;
    
    // Convert device strings to CString - keep them alive for the C call
    let desc_c = device.desc.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let driver_c = device.driver.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let device_id_c = device.device_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let hardware_id_c = device.hardware_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let compatible_id_c = device.compatible_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let upper_filter_c = device.upper_filter.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    
    let vendor_name_c = options.vendor_name.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let device_guid_c = options.device_guid.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let cert_subject_c = options.cert_subject.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());

    let mut device_info = WdiDeviceInfo {
        next: ptr::null_mut(),
        vid: device.vid,
        pid: device.pid,
        is_composite: device.is_composite as c_int,
        mi: device.mi,
        desc: desc_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        driver: driver_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        device_id: device_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        hardware_id: hardware_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        compatible_id: compatible_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        upper_filter: upper_filter_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        driver_version: device.driver_version,
    };

    let mut opts = WdiOptionsPrepareDriver {
        driver_type: options.driver_type.to_c_int(),
        vendor_name: vendor_name_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        device_guid: device_guid_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        disable_cat: options.disable_cat as c_int,
        disable_signing: options.disable_signing as c_int,
        cert_subject: cert_subject_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        use_wcid_driver: options.use_wcid_driver as c_int,
        external_inf: options.external_inf as c_int,
    };

    unsafe {
        let result = wdi_prepare_driver(
            &mut device_info,
            path_c.as_ptr(),
            inf_name_c.as_ptr(),
            &mut opts,
        );
        Error::from_code(result)?;
    }

    Ok(())
}

/// Options for installing a driver, as exposed by libwdi
/// 
/// You can use `default()` to construct.
#[derive(Debug, Clone)]
pub struct InstallDriverOptions {
    pub install_filter_driver: bool,
    /// Timeout in milliseconds to wait for pending installations.
    /// Driver installation often takes around a minute to complete.
    pub pending_install_timeout: u32,
}

impl InstallDriverOptions {
    /// The default timeout for pending installations in milliseconds
    pub const DEFAULT_PENDING_INSTALL_TIMEOUT: u32 = 120000;
}

impl Default for InstallDriverOptions {
    fn default() -> Self {
        InstallDriverOptions {
            install_filter_driver: false,
            pending_install_timeout: Self::DEFAULT_PENDING_INSTALL_TIMEOUT,
        }
    }
}

/// Installs a driver for a device using libwdi.
/// 
/// The driver files must already have been prepared using [`prepare_driver`],
/// or created externally.
/// 
/// # Arguments
/// * `device` - The device for which to install the driver.
/// * `path` - The path where the driver files are located.
/// * `inf_name` - The name of the INF file to use for installation.
/// * `options` - The options to use when installing the driver.
/// 
/// # Errors
/// * Returns an `Error` if the installation fails.
pub fn install_driver(
    device: &Device,
    path: &str,
    inf_name: &str,
    options: &InstallDriverOptions,
) -> Result<(), Error> {
    let path_c = CString::new(path).map_err(|_| Error::InvalidParam)?;
    let inf_name_c = CString::new(inf_name).map_err(|_| Error::InvalidParam)?;

    // Convert device strings to CString
    let desc_c = device.desc.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let driver_c = device.driver.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let device_id_c = device.device_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let hardware_id_c = device.hardware_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let compatible_id_c = device.compatible_id.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());
    let upper_filter_c = device.upper_filter.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok());

    let mut device_info = WdiDeviceInfo {
        next: ptr::null_mut(),
        vid: device.vid,
        pid: device.pid,
        is_composite: device.is_composite as c_int,
        mi: device.mi,
        desc: desc_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        driver: driver_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        device_id: device_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        hardware_id: hardware_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        compatible_id: compatible_id_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        upper_filter: upper_filter_c.as_ref().map_or(ptr::null_mut(), |c| c.as_ptr() as *mut i8),
        driver_version: device.driver_version,
    };

    let mut opts = WdiOptionsInstallDriver {
        hwnd: ptr::null_mut(),
        install_filter_driver: options.install_filter_driver as c_int,
        pending_install_timeout: options.pending_install_timeout,
    };

    unsafe {
        let result = wdi_install_driver(
            &mut device_info,
            path_c.as_ptr(),
            inf_name_c.as_ptr(),
            &mut opts,
        );
        Error::from_code(result)?;
    }

    Ok(())
}

/// Sets the log level for libwdi logging.
pub fn set_log_level(level: LogLevel) -> Result<(), Error> {
    unsafe {
        let result = wdi_set_log_level(level.into());
        Error::from_code(result)
    }
}