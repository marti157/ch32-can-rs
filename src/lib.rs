#![no_std]
#![no_main]

use ch32_metapac as pac;
use pac::{CAN, RCC};

pub enum CanMode {
    Normal,
    Silent,
    Loopback,
    SilentLoopback,
}

struct CanModeRegs {
    /// Loopback mode setting
    lbkm: bool,
    /// Silent mode setting
    silm: bool,
}

impl CanMode {
    fn regs(&self) -> CanModeRegs {
        match self {
            CanMode::Normal => CanModeRegs {
                lbkm: false,
                silm: false,
            },
            CanMode::Silent => CanModeRegs {
                lbkm: false,
                silm: true,
            },
            CanMode::Loopback => CanModeRegs {
                lbkm: true,
                silm: false,
            },
            CanMode::SilentLoopback => CanModeRegs {
                lbkm: true,
                silm: true,
            },
        }
    }
}

pub enum CanFifo {
    Fifo0,
    Fifo1,
}

impl CanFifo {
    fn val(&self) -> usize {
        match self {
            CanFifo::Fifo0 => 0,
            CanFifo::Fifo1 => 1,
        }
    }

    fn val_bool(&self) -> bool {
        match self {
            CanFifo::Fifo0 => false,
            CanFifo::Fifo1 => true,
        }
    }
}

pub enum CanFilterMode {
    /// Matches the incoming ID to a predefined value after applying a predefined bit mask.
    IdMask,
    /// Matches the incoming ID to a predefined set of values.
    IdList,
}

impl CanFilterMode {
    fn val_bool(&self) -> bool {
        match self {
            CanFilterMode::IdMask => false,
            CanFilterMode::IdList => true,
        }
    }
}

/// See table 24-1 of the reference manual for more details on filtering and modes.
pub struct CanFilter {
    /// Filter bank number, 0-27
    bank: usize,
    /// Filter mode, either identifier mask or identifier list
    mode: CanFilterMode,
    /// Values for `STID:EXID:IDE:RTR:0` from msb to lsb to be matched with an incoming message's values.
    /// In IdList mode, value should be a 32-bit id or two 16-bit ids.
    id_value: u32,
    /// Bit mask to be applied to incoming message before comparing it to a predefined value.
    /// In IdList mode, this is used in the same way as `id_value` is.
    id_mask: u32,
}

impl CanFilter {
    /// Offset in `usize` for bank `n` filter register 1
    fn fr_id_value_reg(&self) -> usize {
        self.bank * 2 + 0
    }

    /// Offset in `usize` for bank `n` filter register 2
    fn fr_id_mask_reg(&self) -> usize {
        self.bank * 2 + 1
    }
}

impl Default for CanFilter {
    fn default() -> Self {
        Self {
            bank: 0,
            mode: CanFilterMode::IdMask,
            id_value: 0,
            id_mask: 0,
        }
    }
}

#[derive(Debug)]
pub struct RxMessage {
    /// Message length in bytes, 1-8
    pub length: u8,
    /// Filter bank that matched the message, 0-27
    pub filter: u8,
    /// Identifier used in message
    pub id: u16,
    /// Message data up to `length` bytes, 0 after that
    pub data: [u8; 8],
}

const CAN_INAK_TIMEOUT: u32 = 0x0000FFFF;
const CAN_SJW: u8 = 0b00;
const CAN_TQBS1: u8 = 0b000;
const CAN_TQBS2: u8 = 0b000;
const CAN_PRESCALER: u16 = 12;

pub struct Can {
    fifo: CanFifo,
}

impl Can {
    pub fn new(fifo: CanFifo) -> Self {
        let new_can = Self { fifo };
        new_can.enable();

        new_can
    }

    fn enable(&self) {
        RCC.apb1pcenr().modify(|w| w.set_can1en(true)); // Enable CAN1 peripheral
    }

    /// Initialize CAN peripheral in a certain mode.
    ///
    /// Requires adding a filter before use. See the `add_filter` method.
    pub fn init_mode(&self, mode: CanMode) -> Result<(), &'static str> {
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
            w.set_lbkm(mode.regs().lbkm); // Set silent mode bit from mode
            w.set_silm(mode.regs().silm); // Set loopback mode bit from mode
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

        Ok(())
    }

    pub fn add_filter(&self, filter: CanFilter) {
        CAN.fctlr().modify(|w| w.set_finit(true)); // Enable filter init mode
        CAN.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter in filter bank
        CAN.fscfgr().modify(|w| w.set_fsc(filter.bank, true)); // Set filter scale config to single 32-bit (16-bit not implemented)
        CAN.fr(filter.fr_id_value_reg())
            .write_value(pac::can::regs::Fr(filter.id_value)); // Set filter's id value to match/mask
        CAN.fr(filter.fr_id_mask_reg())
            .write_value(pac::can::regs::Fr(filter.id_mask)); // Set filter's id bits to mask
        CAN.fmcfgr()
            .modify(|w| w.set_fbm(filter.bank, filter.mode.val_bool())); // Set new filter's operating mode
        CAN.fafifor()
            .modify(|w| w.set_ffa(filter.bank, self.fifo.val_bool())); // Associate CAN's FIFO to new filter
        CAN.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter
        CAN.fctlr().modify(|w| w.set_finit(false)); // Exit filter init mode
    }

    pub fn send_message_mbox0(&self, message: u64, stid: u16) {
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

    pub fn receive_message(&self) -> Option<RxMessage> {
        let num_pending_messages = match self.fifo {
            CanFifo::Fifo0 => CAN.rfifo0().read().fmp0(),
            CanFifo::Fifo1 => CAN.rfifo1().read().fmp1(),
        };
        if num_pending_messages == 0 {
            return None;
        }

        let rx_message_unordered: u64 = ((CAN.rxmdhr(self.fifo.val()).read().0 as u64) << 32)
            | CAN.rxmdlr(self.fifo.val()).read().0 as u64;

        let mut message = RxMessage {
            length: CAN.rxmdtr(self.fifo.val()).read().dlc(),
            filter: CAN.rxmdtr(self.fifo.val()).read().fmi(),
            id: CAN.rxmir(self.fifo.val()).read().stid(),
            data: [0; 8],
        };

        message
            .data
            .iter_mut()
            .take(message.length as usize)
            .enumerate()
            .for_each(|(i, byte)| {
                *byte = ((rx_message_unordered >> (i * 8)) & 0xFF) as u8;
            });

        // Release FIFO
        match self.fifo {
            CanFifo::Fifo0 => CAN.rfifo0().modify(|w| w.set_rfom0(true)),
            CanFifo::Fifo1 => CAN.rfifo1().modify(|w| w.set_rfom1(true)),
        }

        Some(message)
    }
}
