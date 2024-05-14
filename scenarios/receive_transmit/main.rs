#![no_std]
#![no_main]

use ch32_can_rs::{Can, CanFifo, CanMode};
use hal::println;
use qingke::riscv;
use {ch32_hal as hal, panic_halt as _};

#[derive(PartialEq)]
enum ScenarioMode {
    Receive,
    Transmit,
}

const SCENARIO_MODE: ScenarioMode = ScenarioMode::Transmit;

#[qingke_rt::entry]
fn main() -> ! {
    hal::debug::SDIPrint::enable();
    let mut config = hal::Config::default();
    config.rcc = hal::rcc::Config::SYSCLK_FREQ_96MHZ_HSI;
    hal::init(config);

    let can = Can::new(CanFifo::Fifo1);

    println!("Starting init CAN normal mode.");

    match can.init_mode(CanMode::Normal) {
        Ok(_) => println!("Initialized CAN in normal mode."),
        Err(msg) => {
            println!("Error initializing CAN: {msg}");
            panic!();
        }
    }

    can.add_filter(Default::default());

    println!("Init CAN normal mode & adding filter OK.");

    if SCENARIO_MODE == ScenarioMode::Transmit {
        let mut msg: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

        loop {
            riscv::asm::delay(50000000);

            let tx_status = can.send_message_no_checks(&msg, 0x317);
            println!("Sent CAN message {:?} with status {:?}", msg, tx_status);

            msg.iter_mut().for_each(|byte| {
                *byte = byte.wrapping_add(1);
            });
        }
    }

    if SCENARIO_MODE == ScenarioMode::Receive {
        loop {
            riscv::asm::delay(50000000);

            match can.receive_message() {
                None => println!("No message."),
                Some(recv_msg) => println!("Received: {:?}", recv_msg),
            }
        }
    }

    println!("Scenario mode not set.");
    loop {}
}
