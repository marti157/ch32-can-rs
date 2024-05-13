### Receive or transmit scenario

This scenario requires a CAN transciever.

In transmit mode, requires a CAN analyzer or receiver chip if you want to view the transmitted frames. In receive mode, it must receive valid CAN frames from a device via the transceiver.

Using `ch32-hal` SDIPrint for debugging.

### Running

Set your chip model in `Cargo.toml` under `ch32-hal` features.

Set transmit or receive mode in `main.rs`, under `SCENARIO_MODE`.

`$ cargo run --release`
