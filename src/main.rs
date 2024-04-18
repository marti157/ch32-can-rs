#![no_std]
#![no_main]

use ch32_hal::usart;
use hal::usart::UartTx;
use qingke::riscv;
use {ch32_hal as hal, panic_halt as _};

#[qingke_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let cfg = usart::Config::default();
    let mut uart = UartTx::new_blocking(p.USART1, p.PA9, cfg).unwrap();

    loop {
        uart.blocking_write(b"Hello world!\r\n").ok();

        riscv::asm::delay(5000000);
    }
}
