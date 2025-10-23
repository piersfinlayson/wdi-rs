// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! # wdi-rs - Windows Driver Installer for Rust
//!
//! This crate provides a Rust API to [libwdi](https://github.com/pbatard/libwdi), the library behind [Zadiq](https://zadig.akeo.ie/), which provides simple Windows device driver installation.  It is particularly useful for installing the WinUSB drivers for USB devices, allowing user-mode applications to communicate with them without needing to write a custom driver.
//!
//! As well as exposing the libwdi primitives, this crate exposes a higher level [`DriverInstaller`] builder API which simplifies common use cases such as:
//! - installing a driver for a specific device by VID/PID
//! - enumerating devices and selecting one based on custom criteria
//! - using a custom INF file for driver installation, allowing more flexibility than the stock libwdi APIs.
//!
//! This is a Windows specific crate, and currently only targets x86 64-bit.
//!
//! ## Features
//!
//! - 🚀 High-level builder API for driver installation
//! - 🔍 USB device enumeration and discovery
//! - 📝 Custom INF file support (embedded or external)
//! - 🛡️ Type-safe bindings to libwdi
//! - 📋 Comprehensive logging support
//! - ✅ x86 64-bit support
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! wdi-rs = "0.1"
//! ```
//!
//! ## Quick Start
//!
//! ### Install WinUSB driver for a specific device
//!
//! The simplest use case - install WinUSB for a device by its VID/PID:
//!
//! ```no_run
//! use wdi_rs::DriverInstaller;
//!
//! fn main() -> Result<(), wdi_rs::Error> {
//!     // Install WinUSB driver for device 1234:5678
//!     // libwdi will automatically generate an appropriate INF file
//!     DriverInstaller::for_device(0x1234, 0x5678)
//!         .install()?;
//!     
//!     println!("Driver installed successfully!");
//!     Ok(())
//! }
//! ```
//!
//! ### Using a custom INF file
//!
//! For production use, you'll typically want to provide your own INF file, allowing you to custom more details of the driver installation:
//!
//! ```no_run
//! use wdi_rs::DriverInstaller;
//!
//! // Embed your INF file at compile time
//! const MY_DEVICE_INF: &[u8] = include_bytes!("..\\inf\\sample.inf");
//!
//! fn main() -> Result<(), wdi_rs::Error> {
//!     DriverInstaller::for_device(0x1234, 0x5678)
//!         .with_inf_data(MY_DEVICE_INF, "..\\inf\\sample.inf")
//!         .install()?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Enumerate devices before installation
//!
//! If you need to discover or select devices interactively:
//!
//! ```no_run
//! use wdi_rs::{create_list, CreateListOptions, DriverInstaller};
//!
//! fn main() -> Result<(), wdi_rs::Error> {
//!     // Enumerate all USB devices
//!     let devices = create_list(CreateListOptions::default())?;
//!     
//!     // Find your specific device
//!     let device = devices.iter()
//!         .find(|d| d.vid == 0x1234 && d.pid == 0x5678)
//!         .ok_or(wdi_rs::Error::NotFound)?;
//!     
//!     println!("Found device: {}", device);
//!     
//!     // Install driver for this specific device
//!     DriverInstaller::for_specific_device(device.clone())
//!         .install()?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Advanced: Custom device selection
//!
//! For more complex device selection logic:
//!
//! ```no_run
//! use wdi_rs::{DriverInstaller, DeviceSelector};
//!
//! fn main() -> Result<(), wdi_rs::Error> {
//!     // Install driver for the first device matching a custom predicate
//!     DriverInstaller::new(DeviceSelector::First(Box::new(|dev| {
//!         dev.vid == 0x1234 && 
//!         dev.desc.as_ref()
//!             .map_or(false, |desc| desc.contains("My Custom Device"))
//!     })))
//!     .install()?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Complete example with error handling and logging
//!
//! ```no_run
//! use wdi_rs::{DriverInstaller, DriverType, Error};
//! use log::{info, error};
//!
//! const MY_DEVICE_INF: &[u8] = include_bytes!("..\\inf\\sample.inf");
//!
//! fn install_driver(vid: u16, pid: u16) -> Result<(), Error> {
//!     info!("Starting driver installation for {:04x}:{:04x}", vid, pid);
//!     
//!     match DriverInstaller::for_device(vid, pid)
//!         .with_inf_data(MY_DEVICE_INF, "..\\inf\\sample.inf")
//!         .with_driver_type(DriverType::WinUsb)
//!         .install()
//!     {
//!         Ok(device) => {
//!             info!("Successfully installed driver for: {}", device);
//!             Ok(())
//!         }
//!         Err(Error::Exists) => {
//!             info!("Driver already installed");
//!             Ok(())
//!         }
//!         Err(Error::NotFound) => {
//!             error!("Device {:04x}:{:04x} not found", vid, pid);
//!             Err(Error::NotFound)
//!         }
//!         Err(e) => {
//!             error!("Failed to install driver: {}", e);
//!             Err(e)
//!         }
//!     }
//! }
//!
//! fn main() {
//!     env_logger::init();
//!     
//!     if let Err(e) = install_driver(0x1234, 0x5678) {
//!         eprintln!("Installation failed: {}", e);
//!         std::process::exit(1);
//!     }
//! }
//! ```
//!
//! ## Low-Level API
//!
//! For cases where you need direct control, wdi-rs also exposes the low-level libwdi functions:
//!
//! ```no_run
//! use wdi_rs::{create_list, prepare_driver, install_driver, CreateListOptions, 
//!           PrepareDriverOptions, InstallDriverOptions, DriverType};
//!
//! fn main() -> Result<(), wdi_rs::Error> {
//!     // Get device list
//!     let devices = create_list(CreateListOptions {
//!         list_all: true,
//!         list_hubs: false,
//!         trim_whitespaces: true,
//!     })?;
//!     
//!     let device = &devices.get(0).ok_or(wdi_rs::Error::NotFound)?;
//!     
//!     // Prepare driver
//!     let mut prepare_opts = PrepareDriverOptions::default();
//!     prepare_opts.driver_type = DriverType::WinUsb;
//!     
//!     prepare_driver(
//!         device,
//!         "C:\\drivers",
//!         "C:\\drivers\\device.inf",
//!         &prepare_opts,
//!     )?;
//!     
//!     // Install driver
//!     install_driver(
//!         device,
//!         "C:\\drivers",
//!         "C:\\drivers\\device.inf",
//!         &InstallDriverOptions::default(),
//!     )?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Platform Support
//!
//! - **Windows 7+** (x64)
//! - Requires administrator privileges for driver installation
//! - This crate only compiles on Windows
//!
//! ## Architecture
//!
//! wdi-rs consists of two layers:
//!
//! 1. **Low-level FFI bindings** to libwdi (`prepare_driver`, `install_driver`, etc.)
//! 2. **High-level API** with `DriverInstaller` builder pattern
//!
//! The high-level API is recommended for most use cases as it handles:
//! - Device enumeration and selection
//! - Temporary file management
//! - INF file handling
//! - Error propagation
//! - Safe cleanup
//!
//! ## Common Use Cases
//!
//! ### Command-line tool for driver installation
//!
//! ```no_run
//! use wdi_rs::DriverInstaller;
//! use std::env;
//!
//! fn main() {
//!     let args: Vec<String> = env::args().collect();
//!     if args.len() != 3 {
//!         eprintln!("Usage: {} <VID> <PID>", args[0]);
//!         std::process::exit(1);
//!     }
//!     
//!     let vid = u16::from_str_radix(&args[1], 16).unwrap();
//!     let pid = u16::from_str_radix(&args[2], 16).unwrap();
//!     
//!     DriverInstaller::for_device(vid, pid)
//!         .install()
//!         .expect("Failed to install driver");
//! }
//! ```
//!
//! ### GUI application with device selection
//!
//! ```no_run
//! use wdi_rs::{create_list, CreateListOptions, DriverInstaller};
//!
//! fn list_devices() -> Result<Vec<String>, wdi_rs::Error> {
//!     let devices = create_list(CreateListOptions::default())?;
//!     Ok(devices.iter()
//!         .map(|d| format!("{:04x}:{:04x} - {}", d.vid, d.pid, 
//!                          d.desc.as_deref().unwrap_or("Unknown")))
//!         .collect())
//! }
//!
//! fn install_for_selected(vid: u16, pid: u16) -> Result<(), wdi_rs::Error> {
//!     DriverInstaller::for_device(vid, pid).install()?;
//!     Ok(())
//! }
//! ```
//!
//! ### Installer package
//!
//! Embed wdi-rs in your application's installer to automatically set up USB drivers:
//!
//! ```no_run
//! use wdi_rs::DriverInstaller;
//!
//! const DEVICE_INF: &[u8] = include_bytes!("..\\inf\\sample.inf");
//!
//! fn setup_usb_driver() -> Result<(), Box<dyn std::error::Error>> {
//!     println!("Setting up USB driver...");
//!     
//!     DriverInstaller::for_device(0x1234, 0x5678)
//!         .with_inf_data(DEVICE_INF, "device.inf")
//!         .install()?;
//!     
//!     println!("USB driver installed successfully!");
//!     Ok(())
//! }
//! ```
//!
//! ## Logging
//!
//! wdi-rs uses the `log` crate for logging. Enable logging in your application:
//!
//! ```no_run
//! use log::LevelFilter;
//!
//! fn main() {
//!     env_logger::Builder::from_default_env()
//!         .filter_level(LevelFilter::Info)
//!         .init();
//!     
//!     // Also set libwdi's log level
//!     wdi_rs::set_log_level(log::max_level().into()).ok();
//!     
//!     // Your code here...
//! }
//! ```
//!
//! ## Safety
//!
//! This crate uses unsafe code to interface with the libwdi C library. All unsafe code is carefully reviewed and encapsulated behind safe APIs. The high-level `DriverInstaller` API is entirely safe Rust.
//!
//! ## License
//!
//! MIT or Apache 2.0 License, at your option - see LICENSE file for details.
//!
//! ## Contributing
//!
//! Contributions are welcome! Please open an issue or PR on GitHub.
//!
//! ## Credits
//!
//! - [libwdi](https://github.com/pbatard/libwdi) by Pete Batard - the underlying C library
//!
//! ## See Also
//!
//! - [libwdi documentation](https://github.com/pbatard/libwdi/wiki)
//! - [rusb](https://crates.io/crates/rusb) - USB library for Rust
//! - [nusb](https://crates.io/crates/nusb) - Modern USB library for Rust

mod ffi;
mod installer;
mod wdi;

pub use installer::{DriverInstaller, DeviceSelector, InfSource, InstallOptions};
pub use wdi::{
    create_list, prepare_driver, install_driver,
    CreateListOptions, Device, DeviceList, PrepareDriverOptions, InstallDriverOptions,
    DriverType, Error, set_log_level,
};
