#![no_std]
#![no_main]

use ch32_can_rs::{hal, nb, Can, CanFifo, CanFilter, CanFrame, CanMode, StandardId};
use hal::println;
use panic_halt as _;
use qingke::riscv;

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
    let p = hal::init(config);

    println!("Creating CAN in normal mode.");

    let can = Can::new(
        p.CAN1,
        p.PB8,
        p.PB9,
        CanFifo::Fifo1,
        CanMode::Normal,
        500_000,
    );
    can.add_filter(CanFilter::accept_all());

    println!("Init CAN normal mode & adding filter OK.");

    if SCENARIO_MODE == ScenarioMode::Transmit {
        let mut msg: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

        loop {
            riscv::asm::delay(50000000);

            let frame = CanFrame::new(StandardId::new(0x317).unwrap(), &msg).unwrap();
            match can.transmit(&frame) {
                Ok(_) => println!("Sent CAN message {:?}", msg),
                Err(nb::Error::WouldBlock) => {
                    println!("Error sending CAN message, mailboxes are full")
                }
                Err(nb::Error::Other(error)) => println!("Error sending CAN message: {error}"),
            };

            msg.iter_mut().for_each(|byte| {
                *byte = byte.wrapping_add(1);
            });
        }
    }

    if SCENARIO_MODE == ScenarioMode::Receive {
        loop {
            riscv::asm::delay(50000000);

            match can.receive() {
                Err(nb::Error::WouldBlock) => println!("No message."),
                Err(nb::Error::Other(error)) => println!("Receive error: {error}"),
                Ok(recv_msg) => println!("Received: {:?}", recv_msg),
            }
        }
    }

    println!("Scenario mode not set.");
    loop {}
}
