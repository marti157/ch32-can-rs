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

    let can = Can::new(CanFifo::Fifo1);

    println!("Starting init CAN silent loopback mode.");

    match can.init_mode(CanMode::SilentLoopback) {
        Ok(_) => println!("Initialized CAN in silent loopback mode."),
        Err(msg) => {
            println!("Error initializing CAN: {msg}");
            panic!();
        }
    }

    can.add_filter(Default::default());

    println!("Init CAN silent loopback mode & adding filter OK.");

    let mut msg: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

    loop {
        riscv::asm::delay(50000000);

        let tx_result = can.send_message_no_checks(&msg, 0x317);
        println!("Sent CAN message: {:?}", tx_result);

        println!("Read CAN message:");
        match can.receive_message() {
            None => println!("No message."),
            Some(recv_msg) => println!("Received: {:?}", recv_msg),
        }

        msg.iter_mut().for_each(|byte| {
            *byte = byte.wrapping_add(1);
        });
    }
}
