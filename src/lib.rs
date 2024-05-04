#![no_std]
#![no_main]

use ch32_metapac as pac;
use pac::{CAN, RCC};

pub struct CanMode {
    /// Loopback mode setting
    lbkm: bool,
    /// Silent mode setting
    silm: bool,
}

impl CanMode {
    pub const NORMAL: CanMode = CanMode {
        lbkm: false,
        silm: false,
    };
    pub const SILENT: CanMode = CanMode {
        lbkm: false,
        silm: true,
    };
    pub const LOOPBACK: CanMode = CanMode {
        lbkm: true,
        silm: false,
    };
    pub const SILENT_LOOPBACK: CanMode = CanMode {
        lbkm: true,
        silm: true,
    };
}

pub enum CanFifo {
    FIFO0 = 0,
    FIFO1 = 1,
}

impl CanFifo {
    fn val(&self) -> usize {
        match self {
            CanFifo::FIFO0 => 0,
            CanFifo::FIFO1 => 1,
        }
    }
}

const CAN_INAK_TIMEOUT: u32 = 0x0000FFFF;
const CAN_SJW: u8 = 0b00;
const CAN_TQBS1: u8 = 0b000;
const CAN_TQBS2: u8 = 0b000;
const CAN_PRESCALER: u16 = 12;

fn init_default_filter() {
    let filter_num = 1;

    CAN.fctlr().modify(|w| w.set_finit(true)); // Enable filter init mode
    CAN.fwr().modify(|w| w.set_fact(filter_num, true)); // Activate filter 1 in filter bank
    CAN.fscfgr().modify(|w| w.set_fsc(filter_num, true)); // Set register of filter 1 to single 32-bit
    CAN.fr(filter_num * 2 + 0)
        .write_value(pac::can::regs::Fr(0b110)); // Masking bits in identifier: 110
    CAN.fr(filter_num * 2 + 1)
        .write_value(pac::can::regs::Fr(0x0000)); // Not masking any bits
    CAN.fmcfgr().modify(|w| w.set_fbm(filter_num, false)); // Set filter 1 to mask bit mode (0)
    CAN.fafifor().modify(|w| w.set_ffa(filter_num, true)); // Associate FIFO1 to filter 1
    CAN.fwr().modify(|w| w.set_fact(filter_num, true)); // Activate filter 1
    CAN.fctlr().modify(|w| w.set_finit(false)); // Exit filter init mode
}

pub fn initialize(mode: CanMode) -> Result<(), &'static str> {
    RCC.apb1pcenr().modify(|w| w.set_can1en(true)); // Enable CAN1 peripheral

    CAN.ctlr().modify(|w| {
        w.set_sleep(false); // Wake up
        w.set_inrq(true); // Request enter init mode
    });

    let mut wait_ack: u32 = 0;
    // Wait until CAN is in init mode
    while !CAN.statr().read().inak() && wait_ack < CAN_INAK_TIMEOUT {
        wait_ack += 1;
    }

    if !CAN.statr().read().inak() {
        return Err("CAN peripheral did not enter initialization mode");
    }

    // CAN baud rate is: CANbps=PCLK1/((TQBS1+TQBS2+3)*(PRESCALER+1))
    CAN.btimr().modify(|w| {
        w.set_brp(CAN_PRESCALER - 1); // Set CAN1 time quantum length
        w.set_ts1(CAN_TQBS1); // Set CAN1 time quantum in bit segment 1
        w.set_ts2(CAN_TQBS2); // Set CAN1 time quantum in bit segment 2
        w.set_sjw(CAN_SJW); // Set CAN1 resync jump width
        w.set_lbkm(mode.lbkm); // Set silent mode bit from mode
        w.set_silm(mode.silm); // Set loopback mode bit from mode
    });

    CAN.ctlr().modify(|w| w.set_inrq(false)); // Request exit init mode

    wait_ack = 0;
    // Wait until CAN is no longer in init mode
    while CAN.statr().read().inak() && wait_ack < CAN_INAK_TIMEOUT {
        wait_ack += 1;
    }

    if CAN.statr().read().inak() {
        return Err("CAN peripheral did not exit initialization mode");
    }

    init_default_filter();

    Ok(())
}

pub fn send_message_mbox0(message: u64, stid: u16) {
    let mailbox_num: usize = 0;
    CAN.txmdtr(mailbox_num).modify(|w| w.set_dlc(8)); // Set message length in bytes
    CAN.txmdhr(mailbox_num)
        .write_value(pac::can::regs::Txmdhr((message >> 32) as u32));
    CAN.txmdlr(mailbox_num)
        .write_value(pac::can::regs::Txmdlr((message & 0xFFFFFFFF) as u32));
    CAN.txmir(mailbox_num)
        .write_value(pac::can::regs::Txmir(0x0)); // Clear CAN1 TXMIR register
    CAN.txmir(mailbox_num).modify(|w| {
        w.set_stid(stid); // Using CAN Standard ID for message
        w.set_txrq(true); // Initiate mailbox transfer request
    });
}

pub fn receive_message_no_checks(fifo: CanFifo) -> Option<u64> {
    let num_pending_messages = match fifo {
        CanFifo::FIFO0 => CAN.rfifo0().read().fmp0(),
        CanFifo::FIFO1 => CAN.rfifo1().read().fmp1(),
    };

    if num_pending_messages == 0 {
        return None;
    }

    // No message length checks
    let received_message: u64 =
        ((CAN.rxmdhr(fifo.val()).read().0 as u64) << 32) | CAN.rxmdlr(fifo.val()).read().0 as u64;

    // Release FIFO
    match fifo {
        CanFifo::FIFO0 => CAN.rfifo0().modify(|w| w.set_rfom0(true)),
        CanFifo::FIFO1 => CAN.rfifo1().modify(|w| w.set_rfom1(true)),
    }

    Some(received_message)
}
