#![no_std]
#![no_main]

use ch32_can_rs::{Can, CanFifo, CanMode};
use hal::println;
use qingke::riscv;
use {ch32_hal as hal, panic_halt as _};

#[qingke_rt::entry]
fn main() -> ! {
    hal::debug::SDIPrint::enable();
    let mut config = hal::Config::default();
    config.rcc = hal::rcc::Config::SYSCLK_FREQ_96MHZ_HSI;
    hal::init(config);

    let can = Can::new(CanFifo::FIFO1);

    println!("Starting init CAN silent loopback mode.");

    match can.init_mode(CanMode::SILENT_LOOPBACK) {
        Ok(_) => println!("Initialized CAN in silent loopback mode."),
        Err(msg) => {
            println!("Error initializing CAN: {msg}");
            panic!();
        }
    }

    println!("Init CAN silent loopback mode OK.");

    let mut msg: u64 = 0x0123456789ABCDEF;

    loop {
        riscv::asm::delay(50000000);

        can.send_message_mbox0(msg, 0x317);
        println!("Sent CAN message.");

        println!("Read CAN message:");
        match can.receive_message_no_checks() {
            None => println!("No message."),
            Some(recv_msg) => println!("0x{:x}", recv_msg),
        }

        msg = msg.wrapping_mul(2);
    }
}
