#![no_std]
#![no_main]

mod error;
mod frame;
mod registers;
mod util;

pub use embedded_can::StandardId;
pub use error::CanError;
pub use frame::CanFrame;
pub use nb;

use ch32_hal as hal;
use ch32_metapac as pac;
use pac::AFIO;
use registers::Registers;

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
    /// Message wasn't sent because all mailboxes were full
    MailboxError,
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

pub struct Can<'d, T: Instance> {
    _peri: hal::PeripheralRef<'d, T>,
    fifo: CanFifo,
}

impl<'d, T: Instance> Can<'d, T> {
    /// Assumes AFIO & PORTB clocks have been enabled by HAL.
    ///
    /// CAN_RX is mapped to PB8, and CAN_TX is mapped to PB9.
    pub fn new(
        peri: impl hal::Peripheral<P = T> + 'd,
        rx: impl hal::Peripheral<P = impl RxPin<T>> + 'd,
        tx: impl hal::Peripheral<P = impl TxPin<T>> + 'd,
        fifo: CanFifo,
        mode: CanMode,
        bitrate: u32,
    ) -> Self {
        hal::into_ref!(peri, rx, tx);

        let this = Self { _peri: peri, fifo };
        T::enable_and_reset(); // Enable CAN peripheral

        rx.set_mode_cnf(
            pac::gpio::vals::Mode::INPUT,
            pac::gpio::vals::Cnf::PULL_IN__AF_PUSH_PULL_OUT,
        );
        tx.set_mode_cnf(
            pac::gpio::vals::Mode::OUTPUT_50MHZ,
            pac::gpio::vals::Cnf::PULL_IN__AF_PUSH_PULL_OUT,
        );
        T::remap(0b10); // CAN_RX is mapped to PB8, and CAN_TX is mapped to PB9

        Registers(T::regs()).enter_init_mode(); // CAN enter initialization mode

        // Configure bit timing parameters and CAN operating mode
        let bit_timings = util::calc_can_timings(T::frequency().0, bitrate).expect(
            "Bit timing parameters weren't satisfied for CAN clock rate and desired bitrate.",
        );
        Registers(T::regs()).set_bit_timing_and_mode(bit_timings, mode);

        Registers(T::regs()).leave_init_mode(); // Exit CAN initialization mode

        this
    }

    pub fn add_filter(&self, filter: CanFilter) {
        Registers(T::regs()).add_filter(filter, &self.fifo);
    }

    /// Puts a frame in the transmit buffer to be sent on the bus.
    ///
    /// If the transmit buffer is full, this function will try to replace a pending
    /// lower priority frame and return the frame that was replaced.
    /// Returns `Err(WouldBlock)` if the transmit buffer is full and no frame can be
    /// replaced.
    pub fn transmit(&self, frame: &CanFrame) -> nb::Result<Option<CanFrame>, CanError> {
        let mailbox_num = match Registers(T::regs()).find_free_mailbox() {
            Some(n) => n,
            None => return Err(nb::Error::WouldBlock),
        };

        Registers(T::regs()).write_frame_mailbox(mailbox_num, frame);

        // Success in readying packet for transmit. No packets can be replaced in the
        // transmit buffer so return None in accordance with embedded-can.
        Ok(None)
    }

    /// Returns a received frame if available.
    pub fn receive(&self) -> nb::Result<CanFrame, CanError> {
        if !Registers(T::regs()).fifo_has_messages_pending(&self.fifo) {
            return nb::Result::Err(nb::Error::WouldBlock);
        }

        let frame = Registers(T::regs()).read_frame_fifo(&self.fifo);

        Ok(frame)
    }
}

/// These trait methods are only usable within the embedded_can context.
/// Under normal use of the [Can] instance,
impl<'d, T> embedded_can::nb::Can for Can<'d, T>
where
    T: Instance,
{
    type Frame = CanFrame;
    type Error = CanError;

    /// Puts a frame in the transmit buffer to be sent on the bus.
    ///
    /// If the transmit buffer is full, this function will try to replace a pending
    /// lower priority frame and return the frame that was replaced.
    /// Returns `Err(WouldBlock)` if the transmit buffer is full and no frame can be
    /// replaced.
    fn transmit(&mut self, frame: &Self::Frame) -> nb::Result<Option<Self::Frame>, Self::Error> {
        Can::transmit(self, frame)
    }

    /// Returns a received frame if available.
    fn receive(&mut self) -> nb::Result<Self::Frame, Self::Error> {
        Can::receive(self)
    }
}

pub trait SealedInstance: hal::RccPeripheral {
    fn regs() -> pac::can::Can;
    /// Either `0b00`, `0b10` or `b11` on CAN1. `0` or `1` on CAN2.
    fn remap(rm: u8) -> ();
}

pub trait Instance: SealedInstance + 'static {}
pub trait RxPin<T: Instance>: hal::gpio::Pin {}
pub trait TxPin<T: Instance>: hal::gpio::Pin {}

impl SealedInstance for hal::peripherals::CAN1 {
    fn regs() -> pac::can::Can {
        pac::CAN1
    }
    fn remap(rm: u8) {
        AFIO.pcfr1().modify(|w| w.set_can1_rm(rm));
    }
}
impl Instance for hal::peripherals::CAN1 {}

impl RxPin<hal::peripherals::CAN1> for hal::peripherals::PB8 {}
impl TxPin<hal::peripherals::CAN1> for hal::peripherals::PB9 {}
