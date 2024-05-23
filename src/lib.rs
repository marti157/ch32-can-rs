#![no_std]
#![no_main]

use ch32_hal as hal;
use ch32_metapac as pac;
use pac::{AFIO, CAN1, GPIOB, RCC};

mod util;

#[derive(PartialEq)]
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
pub enum TxStatus {
    /// Message was sent correctly
    Sent,
    /// Message wasn't sent correctly due to send timeout
    TimeoutError,
    /// Message wasn't sent correctly due to arbitration
    ArbitrationError,
    /// Message wasn't sent correctly due to error
    OtherError,
}

#[derive(Debug)]
pub struct TxResult {
    /// Resulting status of message transmission
    pub status: TxStatus,
    /// Which mailbox was used to send the message, 0-3
    pub mailbox: u8,
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

const CAN_INAK_TIMEOUT: u32 = 0xFFFF;
const CAN_TX_TIMEOUT: u32 = 0xFFF;

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

    /// CAN1_RX is mapped to PB8, and CAN1_TX is mapped to PB9
    fn init_rm_portb(&self) {
        RCC.apb2pcenr().modify(|w| {
            w.set_iopben(true); // Enable clock to PORTB GPIO
            w.set_afioen(true); // Enable AFIO
        });

        AFIO.pcfr1().modify(|w| w.set_can1_rm(0b10));

        GPIOB.cfghr().modify(|w| {
            // PB9 (CAN1_TX) is n = 1 of CFG high
            w.set_cnf(1, pac::gpio::vals::Cnf::PULL_IN__AF_PUSH_PULL_OUT); // Output: PP
            w.set_mode(1, pac::gpio::vals::Mode::OUTPUT_50MHZ);
            // PB8 (CAN1_RX) is n = 0 of CFG high
            w.set_cnf(0, pac::gpio::vals::Cnf::PULL_IN__AF_PUSH_PULL_OUT); // Input: IPU
            w.set_mode(0, pac::gpio::vals::Mode::INPUT);
        });
    }

    /// Initialize CAN peripheral in a certain mode and bitrate (in bps).
    ///
    /// Requires adding a filter before use. See the `add_filter` method.
    pub fn init_mode(&self, mode: CanMode, bitrate: u32) -> Result<(), &'static str> {
        if mode != CanMode::SilentLoopback {
            self.init_rm_portb();
        }

        CAN1.ctlr().modify(|w| {
            w.set_sleep(false); // Wake up
            w.set_inrq(true); // Request enter init mode
        });

        let mut wait_ack: u32 = 0;
        // Wait until CAN is in init mode
        while !CAN1.statr().read().inak() && wait_ack < CAN_INAK_TIMEOUT {
            wait_ack += 1;
        }

        if !CAN1.statr().read().inak() {
            return Err("CAN peripheral did not enter initialization mode");
        }

        // CAN bit rate is: CANbps=PCLK1/((TQBS1+TQBS2+1)*(PRESCALER+1))
        match util::calc_can_timings(hal::rcc::clocks().pclk1.0, bitrate) {
            Some(bt) => {
                let prescaler = u16::from(bt.prescaler) & 0x1FF;
                let seg1 = u8::from(bt.seg1);
                let seg2 = u8::from(bt.seg2) & 0x7F;
                let sync_jump_width = u8::from(bt.sync_jump_width) & 0x7F;
                CAN1.btimr().modify(|w| {
                    w.set_brp(prescaler - 1); // Set CAN1 time quantum length
                    w.set_ts1(seg1 - 1); // Set CAN1 time quantum in bit segment 1
                    w.set_ts2(seg2 - 1); // Set CAN1 time quantum in bit segment 2
                    w.set_sjw(sync_jump_width - 1); // Set CAN1 resync jump width
                    w.set_lbkm(mode.regs().lbkm); // Set silent mode bit from mode
                    w.set_silm(mode.regs().silm); // Set loopback mode bit from mode
                });
            }
            None => return Err(
                "Could not calculate CAN timing parameters for configured clock rate and bit rate",
            ),
        }

        CAN1.ctlr().modify(|w| w.set_inrq(false)); // Request exit init mode

        wait_ack = 0;
        // Wait until CAN is no longer in init mode
        while CAN1.statr().read().inak() && wait_ack < CAN_INAK_TIMEOUT {
            wait_ack += 1;
        }

        if CAN1.statr().read().inak() {
            return Err("CAN peripheral did not exit initialization mode");
        }

        Ok(())
    }

    pub fn add_filter(&self, filter: CanFilter) {
        CAN1.fctlr().modify(|w| w.set_finit(true)); // Enable filter init mode
        CAN1.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter in filter bank
        CAN1.fscfgr().modify(|w| w.set_fsc(filter.bank, true)); // Set filter scale config to single 32-bit (16-bit not implemented)
        CAN1.fr(filter.fr_id_value_reg())
            .write_value(pac::can::regs::Fr(filter.id_value)); // Set filter's id value to match/mask
        CAN1.fr(filter.fr_id_mask_reg())
            .write_value(pac::can::regs::Fr(filter.id_mask)); // Set filter's id bits to mask
        CAN1.fmcfgr()
            .modify(|w| w.set_fbm(filter.bank, filter.mode.val_bool())); // Set new filter's operating mode
        CAN1.fafifor()
            .modify(|w| w.set_ffa(filter.bank, self.fifo.val_bool())); // Associate CAN's FIFO to new filter
        CAN1.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter
        CAN1.fctlr().modify(|w| w.set_finit(false)); // Exit filter init mode
    }

    fn transmit_status_blocking(&self, mailbox_num: usize) -> TxStatus {
        let mut wait_status: u32 = 0;
        while !CAN1.tstatr().read().txok(mailbox_num) && wait_status < CAN_TX_TIMEOUT {
            wait_status += 1;
        }
        if wait_status == CAN_TX_TIMEOUT {
            return TxStatus::TimeoutError;
        }

        let tx_result = CAN1.tstatr().read();
        if tx_result.txok(mailbox_num) {
            return TxStatus::Sent;
        }
        if tx_result.alst(mailbox_num) {
            return TxStatus::ArbitrationError;
        }
        if tx_result.terr(mailbox_num) {
            return TxStatus::OtherError;
        }

        TxStatus::OtherError
    }

    pub fn send_message_no_checks(&self, message: &[u8; 8], stid: u16) -> TxResult {
        // TODO: determine mailbox num depending on emptiness
        let mailbox_num: usize = 0;

        let tx_data_high: u32 = ((message[7] as u32) << 24)
            | ((message[6] as u32) << 16)
            | ((message[5] as u32) << 8)
            | message[4] as u32;
        let tx_data_low: u32 = ((message[3] as u32) << 24)
            | ((message[2] as u32) << 16)
            | ((message[1] as u32) << 8)
            | message[0] as u32;

        CAN1.txmdtr(mailbox_num).modify(|w| w.set_dlc(8)); // Set message length in bytes
        CAN1.txmdhr(mailbox_num)
            .write_value(pac::can::regs::Txmdhr(tx_data_high));
        CAN1.txmdlr(mailbox_num)
            .write_value(pac::can::regs::Txmdlr(tx_data_low));
        CAN1.txmir(mailbox_num)
            .write_value(pac::can::regs::Txmir(0x0)); // Clear CAN1 TXMIR register
        CAN1.txmir(mailbox_num).modify(|w| {
            w.set_stid(stid); // Using CAN Standard ID for message
            w.set_txrq(true); // Initiate mailbox transfer request
        });

        TxResult {
            status: self.transmit_status_blocking(mailbox_num),
            mailbox: mailbox_num as u8,
        }
    }

    pub fn receive_message(&self) -> Option<RxMessage> {
        let num_pending_messages = CAN1.rfifo(self.fifo.val()).read().fmp();
        if num_pending_messages == 0 {
            return None;
        }

        let rx_message_unordered: u64 = ((CAN1.rxmdhr(self.fifo.val()).read().0 as u64) << 32)
            | CAN1.rxmdlr(self.fifo.val()).read().0 as u64;

        let mut message = RxMessage {
            length: CAN1.rxmdtr(self.fifo.val()).read().dlc(),
            filter: CAN1.rxmdtr(self.fifo.val()).read().fmi(),
            id: CAN1.rxmir(self.fifo.val()).read().stid(),
            data: [0; 8],
        };

        // Split rx_message into bytes
        message
            .data
            .iter_mut()
            .take(message.length as usize)
            .enumerate()
            .for_each(|(i, byte)| {
                *byte = ((rx_message_unordered >> (i * 8)) & 0xFF) as u8;
            });

        // Release FIFO
        CAN1.rfifo(self.fifo.val()).modify(|w| w.set_rfom(true));

        Some(message)
    }
}
