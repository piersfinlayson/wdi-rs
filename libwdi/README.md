# libwdi

libwdi is available at https://github.com/pbatard/libwdi

This crate's libwdi binary and other files were built and retrieved from a fork at https://github.com/piersfinlayon/libwdi.  The only changes made to the upstream code were to the CI jobs:
- To disable automatic CI builds.
- To add a build which exposed the necessary files for this crate.

Those files have been checked into this repository under the `/libwdi/` (this) directory.
- libwdi's main header file is located at [include/libwdi.h](include/libwdi.h), as a reference for the FFI bindings.
- The compiled [`libwdi.lib`](lib/libwdi.lib) and debug symbols [`libwdi.pdb`](lib/libwdi.pdb) are linked with this crate's Rust code statically.

## Version

This crate wraps libwdi 1.5.1, with a few extra commits, as it stood at 22 October 2025.

You are welcome to inspect the forked repository and compare to the original.  You are also welcome to build your own version of libwdi from source, following the instructions in the libwdi repository, and replace the files in this crate's `/libwdi/` directory with your own builds.

## License

libwdi is licensed under the GPL and LGPL.  See the libwdi repository for details.  It is likely that this crate's MIT/Apache 2.0 license is only compatible with libwdi under its LGPL license option. Users of this crate receive libwdi under LGPL terms. The author believes that the LGPL requires that users can relink this code with modified versions of libwdi - since this crate is fully open source with documented build procedures, this requirement is satisfied. However, the author is not an attorney and users are responsible for ensuring their use complies with LGPL terms.

libwdi is built with the Microsoft Windows Software Development Kit (SDK) for Windows.  You must accept and agree to the terms of the SDK license to use libwdi.
