#![no_std]
#![no_main]

use ch32_can_rs::{Can, CanFifo, CanMode};
use hal::println;
use qingke::riscv;
use {ch32_hal as hal, panic_halt as _};

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

    loop {
        riscv::asm::delay(50000000);

        println!("Hello world!");
    }
}
