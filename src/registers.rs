pub(crate) struct Registers(pub crate::pac::can::Can);

impl Registers {
    pub fn enter_init_mode(&self) {
        self.0.ctlr().modify(|w| {
            w.set_sleep(false); // Wake up
            w.set_inrq(true); // Request enter init mode
        });

        // Wait until CAN is in init mode
        loop {
            if self.0.statr().read().inak() {
                break;
            }
        }
    }

    pub fn leave_init_mode(&self) {
        self.0.ctlr().modify(|w| w.set_inrq(false)); // Request exit init mode

        // Wait until CAN is no longer in init mode
        loop {
            if !self.0.statr().read().inak() {
                break;
            }
        }
    }

    pub fn set_bit_timing_and_mode(&self, bt: crate::util::NominalBitTiming, mode: crate::CanMode) {
        let prescaler = u16::from(bt.prescaler) & 0x1FF;
        let seg1 = u8::from(bt.seg1);
        let seg2 = u8::from(bt.seg2) & 0x7F;
        let sync_jump_width = u8::from(bt.sync_jump_width) & 0x7F;
        self.0.btimr().modify(|w| {
            w.set_brp(prescaler - 1); // Set CAN clock prescaler
            w.set_ts1(seg1 - 1); // Set CAN time quantum in bit segment 1
            w.set_ts2(seg2 - 1); // Set CAN time quantum in bit segment 2
            w.set_sjw(sync_jump_width - 1); // Set CAN resync jump width
            w.set_lbkm(mode.regs().lbkm); // Set silent mode bit from mode
            w.set_silm(mode.regs().silm); // Set loopback mode bit from mode
        });
    }

    pub fn add_filter(&self, filter: crate::CanFilter, associate_fifo: &crate::CanFifo) {
        self.0.fctlr().modify(|w| w.set_finit(true)); // Enable filter init mode
        self.0.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter in filter bank
        self.0.fscfgr().modify(|w| w.set_fsc(filter.bank, true)); // Set filter scale config to single 32-bit (16-bit not implemented)
        self.0
            .fr(filter.fr_id_value_reg())
            .write_value(crate::pac::can::regs::Fr(filter.id_value)); // Set filter's id value to match/mask
        self.0
            .fr(filter.fr_id_mask_reg())
            .write_value(crate::pac::can::regs::Fr(filter.id_mask)); // Set filter's id bits to mask
        self.0
            .fmcfgr()
            .modify(|w| w.set_fbm(filter.bank, filter.mode.val_bool())); // Set new filter's operating mode
        self.0
            .fafifor()
            .modify(|w| w.set_ffa(filter.bank, associate_fifo.val_bool())); // Associate CAN's FIFO to new filter
        self.0.fwr().modify(|w| w.set_fact(filter.bank, true)); // Activate new filter
        self.0.fctlr().modify(|w| w.set_finit(false)); // Exit filter init mode
    }
}
