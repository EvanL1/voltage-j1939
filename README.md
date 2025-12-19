# voltage_j1939

[![Crates.io](https://img.shields.io/crates/v/voltage_j1939.svg)](https://crates.io/crates/voltage_j1939)
[![Documentation](https://docs.rs/voltage_j1939/badge.svg)](https://docs.rs/voltage_j1939)
[![License](https://img.shields.io/crates/l/voltage_j1939.svg)](LICENSE-MIT)

SAE J1939 protocol decoder for Rust. Provides PGN/SPN database and CAN frame parsing for heavy-duty vehicles and industrial equipment.

## Features

- **Zero dependencies** - Pure Rust, no external crates required
- **Built-in SPN database** - 60+ SPNs across 12+ PGNs for engine/generator monitoring
- **CAN ID parsing** - Parse and build 29-bit extended J1939 CAN IDs
- **Bit-level decoding** - Extract values with scale, offset, and bit field support
- **"Not available" detection** - Automatic handling of J1939 special values (0xFF, 0xFFFF, etc.)

## Installation

```toml
[dependencies]
voltage_j1939 = "0.1"
```

## Quick Start

```rust
use voltage_j1939::{decode_frame, parse_can_id};

// Parse a J1939 CAN frame (EEC1 from SA=0x00)
let can_id = 0x0CF00400u32;
let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];

// Decode all SPNs in the frame
let decoded = decode_frame(can_id, &data);
for spn in decoded {
    println!("{}: {} {}", spn.name, spn.value, spn.unit);
}

// Parse CAN ID components
let id = parse_can_id(can_id);
println!("PGN: {}, SA: 0x{:02X}", id.pgn, id.source_address);
```

## Decoding Individual SPNs

```rust
use voltage_j1939::{decode_spn, get_spn_def};

// Get SPN definition for Engine Coolant Temperature
let spn_def = get_spn_def(110).unwrap();

// Decode from raw CAN data
let data = [130u8, 0, 0, 0, 0, 0, 0, 0];  // Raw value 130
if let Some(value) = decode_spn(&data, spn_def) {
    // value = 130 * 1.0 + (-40) = 90°C
    println!("Coolant temp: {}°C", value);
}
```

## Supported PGNs

| PGN | Name | Description |
|-----|------|-------------|
| 61444 | EEC1 | Electronic Engine Controller 1 |
| 61443 | EEC2 | Electronic Engine Controller 2 |
| 65270 | EEC3 | Electronic Engine Controller 3 |
| 65262 | ET1 | Engine Temperature 1 |
| 65263 | EFL/P1 | Engine Fluid Level/Pressure 1 |
| 65270 | IC1 | Inlet/Exhaust Conditions 1 |
| 65271 | VEP1 | Vehicle Electrical Power 1 |
| 65269 | AMB | Ambient Conditions |
| 65266 | LFE | Fuel Economy |
| 65253 | HOURS | Engine Hours/Revolutions |
| 65257 | FC | Fuel Consumption |
| 65259 | VH | Vehicle Hours |
| 65276 | DD | Dash Display |
| 65265 | CCVS | Cruise Control/Vehicle Speed |

## J1939 CAN ID Format

J1939 uses 29-bit extended CAN IDs:

```
| Priority | R | DP | PF | PS/DA | SA |
|   3 bit  |1b | 1b | 8b |  8b   | 8b |
```

- **Priority**: Message priority (0-7, lower is higher)
- **DP**: Data Page (0 or 1)
- **PF**: PDU Format (determines PDU1 vs PDU2)
- **PS/DA**: PDU Specific or Destination Address
- **SA**: Source Address

### PDU Format

- **PDU1** (PF < 240): Peer-to-peer messages, PS is destination address
- **PDU2** (PF >= 240): Broadcast messages, PS is part of PGN

## Building Request PGN Frames

```rust
use voltage_j1939::build_request_pgn;

// Request Engine Hours (PGN 65253) from ECU at address 0x00
let (can_id, data) = build_request_pgn(0xFE, 0x00, 65253);
// can_id = 0x18EA00FE
// data = [0xE5, 0xFE, 0x00] (PGN in little-endian)
```

## Database Statistics

```rust
use voltage_j1939::{database_stats, list_supported_pgns};

let (spn_count, pgn_count) = database_stats();
println!("Database: {} SPNs across {} PGNs", spn_count, pgn_count);

for pgn in list_supported_pgns() {
    println!("PGN {}", pgn);
}
```

## Integration with socketcan

This crate handles J1939 protocol decoding only. For CAN bus communication, use [socketcan](https://crates.io/crates/socketcan) or similar:

```rust
use socketcan::CanSocket;
use voltage_j1939::{decode_frame, parse_can_id};

let socket = CanSocket::open("can0")?;
loop {
    let frame = socket.read_frame()?;
    if frame.is_extended() {
        let id = parse_can_id(frame.id());
        let decoded = decode_frame(frame.id(), frame.data());
        for spn in decoded {
            println!("[SA=0x{:02X}] {}: {} {}",
                id.source_address, spn.name, spn.value, spn.unit);
        }
    }
}
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
