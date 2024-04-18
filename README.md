Serial on the CH32V20X line using the [ch32v2 PAC](https://crates.io/crates/ch32v2).

Sample from the [ch32-rs](https://github.com/ch32-rs) team. Tested using Rust `1.76.0`.

## Wiring

`PA0` needs to be wired to any of the `LED` pins.

## Requirements

You must have an OpenOCD binary patched to support Wlink. WCH has published the source to their version of [OpenOCD](https://github.com/openwch/openocd_wch), which can be built.

Make sure to update the environment variables to reflect your binary.

## Notes

When running a debug build, the clock speed is around 8x slower.

## Setup

`$ rustup target add riscv32imac-unknown-none-elf`

## Building

`$ make`

For a debug build:

`$ make DEBUG=1`

## Flasing

Connect the board through the LinkE adapter

`$ make flash`

To flash a debug build:

`$ make flash DEBUG=1`

## Debugging

Debugging not only requires OpenOCD, but a RISC-V GNU Toolchain (for GDB). You can build one from riscv-collab, or get a pre-built one by xPack or others.

Start the GDB server in one session:

`$ make debug`

And connect to it through another:

`$ make connect`

## LICENSE

MIT
