OPENOCD_BIN = ./openocd
GDB_BIN = riscv-none-embed-gdb
DEBUG = 0
TARGET = ch32-can-rs
FLAGS =
DEPS = src/main.rs

ifeq ($(DEBUG), 0)
	FLAGS += --release
	TARGET_DIR = target/riscv32imac-unknown-none-elf/release
else
	TARGET_DIR = target/riscv32imac-unknown-none-elf/debug
endif

$(TARGET_DIR)/$(TARGET): Cargo.toml .cargo/config.toml build.rs $(DEPS)
	cargo build $(FLAGS)

flash: $(TARGET_DIR)/$(TARGET)
	sudo $(OPENOCD_BIN) -f wch-riscv.cfg -c "program $(TARGET_DIR)/$(TARGET)" -c wlink_reset_resume -c exit

debug:
	$(GDB_BIN) $(TARGET_DIR)/$(TARGET) -ex "target extended-remote :3333"

connect:
	sudo $(OPENOCD_BIN) -f wch-riscv.cfg
