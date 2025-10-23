// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! High-level API for installing USB drivers on Windows using libwdi.
//!
//! This module provides a builder-pattern interface for installing drivers,
//! with support for custom INF files, device selection strategies, and
//! comprehensive error handling.
//!
//! # Examples
//!
//! ## Simple VID/PID installation with embedded INF
//!
//! ```no_run
//! use wdi_rs::{DriverInstaller, InfSource};
//!
//! const MY_INF: &[u8] = include_bytes!("..\\inf\\sample.inf");
//!
//! DriverInstaller::for_device(0x1234, 0x5678)
//!     .with_inf_data(MY_INF, "my_device.inf")
//!     .install()?;
//! # Ok::<(), wdi_rs::Error>(())
//! ```
//!
//! ## Installation with libwdi-generated INF
//!
//! ```no_run
//! use wdi_rs::DriverInstaller;
//!
//! // Uses libwdi to generate the INF automatically
//! DriverInstaller::for_device(0x1234, 0x5678)
//!     .install()?;
//! # Ok::<(), wdi_rs::Error>(())
//! ```
//!
//! ## Enumerate devices then install
//!
//! ```no_run
//! use wdi_rs::{create_list, CreateListOptions, DriverInstaller};
//!
//! let devices = create_list(CreateListOptions::default())?;
//! let device = devices.iter()
//!     .find(|d| d.vid == 0x1234 && d.pid == 0x5678)
//!     .expect("Device not found");
//!
//! DriverInstaller::for_specific_device(device.clone())
//!     .install()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Custom device selection
//!
//! ```no_run
//! use wdi_rs::{DriverInstaller, DeviceSelector};
//!
//! DriverInstaller::new(DeviceSelector::First(Box::new(|dev| {
//!     dev.vid == 0x1234 && dev.desc.as_ref().expect("Device description not found").contains("My Device")
//! })))
//! .install()?;
//! # Ok::<(), wdi_rs::Error>(())
//! ```

use std::fmt;
use std::fs;
use std::path::PathBuf;
use log::{debug, error, info, trace, warn};
use tempfile::TempDir;

// Import the low-level wdi types
use crate::{
    create_list, prepare_driver, install_driver, 
    CreateListOptions, PrepareDriverOptions, InstallDriverOptions,
    Device, DriverType, Error as WdiError,
};

/// Strategy for selecting which USB device to install a driver for.
pub enum DeviceSelector {
    /// Select a device by USB Vendor ID and Product ID.
    ///
    /// If multiple devices match, the first one found will be used.
    VidPid { 
        /// USB Vendor ID
        vid: u16, 
        /// USB Product ID
        pid: u16 
    },
    
    /// Select the first device matching a predicate function.
    ///
    /// The predicate receives a reference to each device and returns `true`
    /// if it should be selected.
    First(Box<dyn Fn(&Device) -> bool>),
    
    /// Use a specific device that was previously enumerated.
    ///
    /// This is useful when you've already called [`create_list`] and want
    /// to install a driver for a specific device from that list.
    Specific(Device),
}

impl fmt::Debug for DeviceSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VidPid { vid, pid } => write!(f, "VidPid({:04x}:{:04x})", vid, pid),
            Self::First(_) => write!(f, "First(<predicate>)"),
            Self::Specific(dev) => write!(f, "Specific({})", dev),
        }
    }
}

/// Source for the INF file used during driver installation.
#[derive(Clone)]
pub enum InfSource {
    /// Use an embedded INF file from memory.
    ///
    /// The data will be written to a temporary directory during installation.
    Embedded { 
        /// Raw INF file contents
        data: Vec<u8>, 
        /// Filename to use when writing the INF file
        filename: String 
    },
    
    /// Use an existing INF file from the filesystem.
    External { 
        /// Path to the INF file
        path: PathBuf 
    },
    
    /// Let libwdi generate the INF file automatically.
    ///
    /// This is the default and simplest option if you don't need
    /// custom INF file contents.
    Generated,
}

impl fmt::Debug for InfSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedded { data, filename } => 
                write!(f, "Embedded({} bytes, {})", data.len(), filename),
            Self::External { path } => 
                write!(f, "External({})", path.display()),
            Self::Generated => 
                write!(f, "Generated"),
        }
    }
}

impl Default for InfSource {
    fn default() -> Self {
        Self::Generated
    }
}

/// Options for driver installation.
///
/// Wraps the low-level [`PrepareDriverOptions`] and [`InstallDriverOptions`]
/// with sensible defaults.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Options for driver preparation phase
    pub prepare_opts: PrepareDriverOptions,
    /// Options for driver installation phase
    pub install_opts: InstallDriverOptions,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            prepare_opts: PrepareDriverOptions::default(),
            install_opts: InstallDriverOptions::default(),
        }
    }
}

/// High-level builder for installing USB drivers.
///
/// This provides a fluent interface for configuring and executing driver
/// installation operations. Use one of the constructor methods to create
/// an instance, configure it with the builder methods, then call [`install`]
/// to perform the installation.
///
/// [`install`]: DriverInstaller::install
///
/// # Examples
///
/// ```no_run
/// use wdi_rs::{DriverInstaller, DriverType};
///
/// DriverInstaller::for_device(0x1234, 0x5678)
///     .with_driver_type(DriverType::WinUsb)
///     .install()?;
/// # Ok::<(), wdi_rs::Error>(())
/// ```
pub struct DriverInstaller {
    device_selector: DeviceSelector,
    driver_type: DriverType,
    inf_source: InfSource,
    options: InstallOptions,
}

impl DriverInstaller {
    /// Create a new installer with a custom device selector.
    ///
    /// For most cases, prefer [`for_device`] or [`for_specific_device`].
    ///
    /// [`for_device`]: DriverInstaller::for_device
    /// [`for_specific_device`]: DriverInstaller::for_specific_device
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::{DriverInstaller, DeviceSelector};
    ///
    /// let installer = DriverInstaller::new(
    ///     DeviceSelector::First(Box::new(|dev| {
    ///         dev.vid == 0x1234 && dev.desc.as_ref()
    ///             .map_or(false, |m| m.contains("ACME"))
    ///     }))
    /// );
    /// ```
    pub fn new(device_selector: DeviceSelector) -> Self {
        debug!("Creating DriverInstaller with selector: {:?}", device_selector);
        Self {
            device_selector,
            driver_type: DriverType::WinUsb,
            inf_source: InfSource::default(),
            options: InstallOptions::default(),
        }
    }
    
    /// Create an installer for a device with the specified VID and PID.
    ///
    /// If multiple devices match, the first one found will be used.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::DriverInstaller;
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678);
    /// ```
    pub fn for_device(vid: u16, pid: u16) -> Self {
        info!("Creating installer for VID:PID {:04x}:{:04x}", vid, pid);
        Self::new(DeviceSelector::VidPid { vid, pid })
    }
    
    /// Create an installer for a specific device.
    ///
    /// This is useful when you've already enumerated devices with [`create_list`]
    /// and want to install a driver for a specific one.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::{create_list, CreateListOptions, DriverInstaller};
    ///
    /// let devices = create_list(CreateListOptions::default()).expect("Failed to list devices");
    /// let device = &devices.get(0).expect("No devices found");
    /// let installer = DriverInstaller::for_specific_device(device.clone());
    /// ```
    pub fn for_specific_device(device: Device) -> Self {
        info!("Creating installer for specific device: {}", device);
        Self::new(DeviceSelector::Specific(device))
    }
    
    /// Set the INF source to embedded data.
    ///
    /// The provided data will be written to a temporary file during installation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::DriverInstaller;
    ///
    /// const INF_DATA: &[u8] = include_bytes!("..\\inf\\sample.inf");
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .with_inf_data(INF_DATA, "my_device.inf");
    /// ```
    pub fn with_inf_data(mut self, data: &[u8], filename: impl Into<String>) -> Self {
        let filename = filename.into();
        debug!("Setting INF source to embedded data: {} ({} bytes)", filename, data.len());
        self.inf_source = InfSource::Embedded {
            data: data.to_vec(),
            filename,
        };
        self
    }
    
    /// Set the INF source to an external file.
    ///
    /// The file must exist and be readable at installation time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::DriverInstaller;
    /// use std::path::PathBuf;
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .with_inf_file(PathBuf::from("C:\\drivers\\my_device.inf"));
    /// ```
    pub fn with_inf_file(mut self, path: PathBuf) -> Self {
        debug!("Setting INF source to external file: {}", path.display());
        self.inf_source = InfSource::External { path };
        self
    }
    
    /// Set the driver type to install.
    ///
    /// Defaults to [`DriverType::WinUsb`] if not specified.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::{DriverInstaller, DriverType};
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .with_driver_type(DriverType::LibUsb0);
    /// ```
    pub fn with_driver_type(mut self, driver_type: DriverType) -> Self {
        debug!("Setting driver type to: {:?}", driver_type);
        self.driver_type = driver_type;
        self
    }
    
    /// Set custom options for the driver preparation phase.
    ///
    /// Note: The `external_inf` field will be automatically set based on
    /// the [`InfSource`] and any value you set will be overridden. A warning
    /// will be logged if you attempt to set it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::{DriverInstaller, PrepareDriverOptions};
    ///
    /// let mut opts = PrepareDriverOptions::default();
    /// // Configure opts as needed...
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .with_prepare_options(opts);
    /// ```
    pub fn with_prepare_options(mut self, opts: PrepareDriverOptions) -> Self {
        debug!("Setting custom prepare options");
        self.options.prepare_opts = opts;
        self
    }
    
    /// Set custom options for the driver installation phase.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::{DriverInstaller, InstallDriverOptions};
    ///
    /// let mut opts = InstallDriverOptions::default();
    /// // Configure opts as needed...
    ///
    /// let installer = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .with_install_options(opts);
    /// ```
    pub fn with_install_options(mut self, opts: InstallDriverOptions) -> Self {
        debug!("Setting custom install options");
        self.options.install_opts = opts;
        self
    }
    
    /// Perform the driver installation.
    ///
    /// This will:
    /// 1. Find the target device (if not already specified)
    /// 2. Check if a driver is already installed
    /// 3. Prepare the driver files
    /// 4. Install the driver
    ///
    /// Returns the [`Device`] for the device that was installed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The device cannot be found
    /// - A non-WinUSB driver is already installed
    /// - Driver preparation fails
    /// - Driver installation fails
    /// - File I/O operations fail
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wdi_rs::DriverInstaller;
    ///
    /// let device = DriverInstaller::for_device(0x1234, 0x5678)
    ///     .install()?;
    ///
    /// println!("Installed driver for device: {}", device);
    /// # Ok::<(), wdi_rs::Error>(())
    /// ```
    pub fn install(self) -> Result<Device, WdiError> {
        info!("Starting driver installation");
        debug!("Configuration: selector={:?}, driver_type={:?}, inf_source={:?}", 
               self.device_selector, self.driver_type, self.inf_source);
        
        let device = self.find_device()?;
        self.check_existing_driver(&device)?;
        self.prepare_and_install(device)
    }
    
    /// Find the target device based on the selector.
    fn find_device(&self) -> Result<Device, WdiError> {
        debug!("Finding target device");
        
        match &self.device_selector {
            DeviceSelector::Specific(device) => {
                debug!("Using pre-selected device: {}", device);
                Ok(device.clone())
            }
            
            DeviceSelector::VidPid { vid, pid } => {
                debug!("Enumerating USB devices");
                let opts = CreateListOptions {
                    list_all: true,
                    list_hubs: false,
                    trim_whitespaces: true,
                };
                
                let devices = create_list(opts)?;
                trace!("Found {} USB devices", devices.len());
                
                if devices.is_empty() {
                    error!("No USB devices found on the system");
                    return Err(WdiError::NotFound);
                }
                
                let matching: Vec<_> = devices.iter()
                    .filter(|d| d.vid == *vid && d.pid == *pid)
                    .collect();
                
                if matching.is_empty() {
                    error!("No USB devices found with VID:PID {:04x}:{:04x}", vid, pid);
                    return Err(WdiError::NotFound);
                }
                
                if matching.len() > 1 {
                    warn!("Multiple USB devices found with VID:PID {:04x}:{:04x}", vid, pid);
                    info!("Using first device found");
                }
                
                let device = matching[0].clone();
                info!("Found target device: {}", device);
                Ok(device)
            }
            
            DeviceSelector::First(predicate) => {
                debug!("Enumerating USB devices with predicate filter");
                let opts = CreateListOptions {
                    list_all: true,
                    list_hubs: false,
                    trim_whitespaces: true,
                };
                
                let devices = create_list(opts)?;
                trace!("Found {} USB devices", devices.len());
                
                if devices.is_empty() {
                    error!("No USB devices found on the system");
                    return Err(WdiError::NotFound);
                }
                
                let device = devices.iter()
                    .find(|d| predicate(d))
                    .ok_or_else(|| {
                        error!("No device matched the predicate");
                        WdiError::NotFound
                    })?
                    .clone();
                
                info!("Found target device: {}", device);
                Ok(device)
            }
        }
    }
    
    /// Check if the device already has a driver installed.
    fn check_existing_driver(&self, device: &Device) -> Result<(), WdiError> {
        debug!("Checking existing driver for device: {}", device);
        
        if let Some(driver) = &device.driver {
            if driver.starts_with("WinUSB") {
                info!("Device already has WinUSB driver installed - nothing to do");
                return Err(WdiError::Exists);
            } else {
                error!("Device already has a non-WinUSB driver installed: {}", driver);
                error!("Cannot replace existing driver - manual uninstall required");
                return Err(WdiError::Exists);
            }
        }
        
        debug!("Device has no driver installed - proceeding");
        Ok(())
    }
    
    /// Prepare and install the driver.
    fn prepare_and_install(mut self, device: Device) -> Result<Device, WdiError> {
        info!("Preparing and installing driver for device: {}", device);
        
        // Determine if we need external INF and set up paths
        let (driver_path, inf_path, _temp_dir) = match &self.inf_source {
            InfSource::Embedded { data, filename } => {
                debug!("Setting up embedded INF file");
                let temp_dir = TempDir::new()
                    .map_err(|e| {
                        error!("Failed to create temporary directory: {}", e);
                        WdiError::Resource
                    })?;
                
                let driver_path = temp_dir.path().to_str()
                    .ok_or_else(|| {
                        error!("Failed to get temporary directory path");
                        WdiError::InvalidParam
                    })?
                    .to_string();
                
                let inf_file_path = temp_dir.path().join(filename);
                debug!("Writing INF file to: {}", inf_file_path.display());
                
                fs::write(&inf_file_path, data)
                    .map_err(|e| {
                        error!("Failed to write INF file: {}", e);
                        WdiError::Resource
                    })?;
                
                let inf_path = inf_file_path.to_str()
                    .ok_or_else(|| {
                        error!("Failed to convert INF path to string");
                        WdiError::InvalidParam
                    })?
                    .to_string();
                
                info!("INF file written successfully");
                (driver_path, inf_path, Some(temp_dir))
            }
            
            InfSource::External { path } => {
                debug!("Using external INF file: {}", path.display());
                
                if !path.exists() {
                    error!("External INF file does not exist: {}", path.display());
                    return Err(WdiError::NotFound);
                }
                
                let driver_path = path.parent()
                    .ok_or_else(|| {
                        error!("Invalid external INF path - no parent directory");
                        WdiError::InvalidParam
                    })?
                    .to_str()
                    .ok_or_else(|| {
                        error!("Failed to convert driver path to string");
                        WdiError::InvalidParam
                    })?
                    .to_string();
                
                let inf_path = path.to_str()
                    .ok_or_else(|| {
                        error!("Failed to convert INF path to string");
                        WdiError::InvalidParam
                    })?
                    .to_string();
                
                (driver_path, inf_path, None)
            }
            
            InfSource::Generated => {
                debug!("Using libwdi-generated INF file");
                let temp_dir = TempDir::new()
                    .map_err(|e| {
                        error!("Failed to create temporary directory: {}", e);
                        WdiError::Resource
                    })?;
                
                let driver_path = temp_dir.path().to_str()
                    .ok_or_else(|| {
                        error!("Failed to get temporary directory path");
                        WdiError::InvalidParam
                    })?
                    .to_string();
                
                // For generated INF, libwdi will create it
                let inf_path = format!("{}\\generated.inf", driver_path);
                
                (driver_path, inf_path, Some(temp_dir))
            }
        };
        
        // Set external_inf based on INF source, warning if user tried to set it
        let should_use_external_inf = !matches!(self.inf_source, InfSource::Generated);
        
        if self.options.prepare_opts.external_inf != should_use_external_inf {
            warn!("Overriding prepare_opts.external_inf (was {}, setting to {}) based on InF source",
                  self.options.prepare_opts.external_inf, should_use_external_inf);
        }
        
        self.options.prepare_opts.external_inf = should_use_external_inf;
        self.options.prepare_opts.driver_type = self.driver_type;
        
        // Prepare the driver
        debug!("Preparing driver in: {}", driver_path);
        debug!("INF path: {}", inf_path);
        
        prepare_driver(
            &device,
            &driver_path,
            &inf_path,
            &self.options.prepare_opts,
        ).map_err(|e| {
            error!("Failed to prepare driver: {}", e);
            e
        })?;
        
        info!("Driver prepared successfully");
        
        // Install the driver
        debug!("Installing driver");
        
        install_driver(
            &device,
            &driver_path,
            &inf_path,
            &self.options.install_opts,
        ).map_err(|e| {
            error!("Failed to install driver: {}", e);
            e
        })?;
        
        info!("Driver installed successfully");
        
        // Keep temp_dir alive until here so it doesn't get cleaned up prematurely
        drop(_temp_dir);
        
        Ok(device)
    }
}

impl fmt::Debug for DriverInstaller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriverInstaller")
            .field("device_selector", &self.device_selector)
            .field("driver_type", &self.driver_type)
            .field("inf_source", &self.inf_source)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_device_selector_vid_pid() {
        let installer = DriverInstaller::for_device(0x1234, 0x5678);
        match installer.device_selector {
            DeviceSelector::VidPid { vid, pid } => {
                assert_eq!(vid, 0x1234);
                assert_eq!(pid, 0x5678);
            }
            _ => panic!("Wrong selector type"),
        }
    }
    
    #[test]
    fn test_builder_pattern() {
        let installer = DriverInstaller::for_device(0x1234, 0x5678)
            .with_driver_type(DriverType::LibUsb0)
            .with_inf_data(b"test data", "test.inf");
        
        assert!(matches!(installer.driver_type, DriverType::LibUsb0));
        assert!(matches!(installer.inf_source, InfSource::Embedded { .. }));
    }
    
    #[test]
    fn test_default_inf_source() {
        let installer = DriverInstaller::for_device(0x1234, 0x5678);
        assert!(matches!(installer.inf_source, InfSource::Generated));
    }
}
