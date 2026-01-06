//! SAE J1939 protocol decoder.
//!
//! This crate provides a pure Rust implementation for decoding SAE J1939 CAN frames,
//! including a built-in database of common PGNs and SPNs used in heavy-duty vehicles
//! and industrial equipment.
//!
//! # Features
//!
//! - **Zero dependencies**: Pure Rust, no external crates required
//! - **Built-in SPN database**: 60+ SPNs across 12+ PGNs for engine/generator monitoring
//! - **CAN ID parsing**: Parse and build 29-bit extended J1939 CAN IDs
//! - **Bit-level decoding**: Extract values with scale, offset, and bit field support
//! - **"Not available" detection**: Automatic handling of J1939 special values
//!
//! # Quick Start
//!
//! ```rust
//! use voltage_j1939::{decode_frame, parse_can_id};
//!
//! // Parse a J1939 CAN frame
//! let can_id = 0x0CF00400u32;  // EEC1 from SA=0x00
//! let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];
//!
//! // Decode all SPNs in the frame
//! let decoded = decode_frame(can_id, &data);
//! for spn in decoded {
//!     println!("{}: {} {}", spn.name, spn.value, spn.unit);
//! }
//!
//! // Parse CAN ID components
//! let id = parse_can_id(can_id);
//! println!("PGN: {}, SA: 0x{:02X}", id.pgn, id.source_address);
//! ```
//!
//! # Decoding Individual SPNs
//!
//! ```rust
//! use voltage_j1939::{decode_spn, get_spn_def};
//!
//! // Get SPN definition
//! let spn_def = get_spn_def(110).unwrap();  // Engine Coolant Temperature
//!
//! // Decode from raw data
//! let data = [130u8, 0, 0, 0, 0, 0, 0, 0];  // Raw value 130
//! if let Some(value) = decode_spn(&data, spn_def) {
//!     // value = 130 * 1.0 + (-40) = 90°C
//!     println!("Coolant temp: {}°C", value);
//! }
//! ```
//!
//! # Supported PGNs
//!
//! | PGN | Name | Description |
//! |-----|------|-------------|
//! | 61444 | EEC1 | Electronic Engine Controller 1 |
//! | 61443 | EEC2 | Electronic Engine Controller 2 |
//! | 65270 | EEC3 | Electronic Engine Controller 3 |
//! | 65262 | ET1 | Engine Temperature 1 |
//! | 65263 | EFL/P1 | Engine Fluid Level/Pressure 1 |
//! | 65270 | IC1 | Inlet/Exhaust Conditions 1 |
//! | 65271 | VEP1 | Vehicle Electrical Power 1 |
//! | 65269 | AMB | Ambient Conditions |
//! | 65266 | LFE | Fuel Economy |
//! | 65253 | HOURS | Engine Hours/Revolutions |
//! | 65257 | FC | Fuel Consumption |
//! | 65259 | VH | Vehicle Hours |
//! | 65276 | DD | Dash Display |
//! | 65265 | CCVS | Cruise Control/Vehicle Speed |
//!
//! # J1939 CAN ID Format
//!
//! J1939 uses 29-bit extended CAN IDs with the following structure:
//!
//! ```text
//! | Priority | R | DP | PF | PS/DA | SA |
//! |   3 bit  |1b | 1b | 8b |  8b   | 8b |
//! ```
//!
//! - **Priority**: Message priority (0-7, lower is higher)
//! - **R**: Reserved (always 0)
//! - **DP**: Data Page
//! - **PF**: PDU Format (determines PDU1 vs PDU2)
//! - **PS/DA**: PDU Specific or Destination Address
//! - **SA**: Source Address

#![deny(missing_docs)]
// Note: We allow unsafe in decoder.rs for performance-critical hot paths
// after bounds checking. All unsafe is minimal and well-documented.

pub mod database;
pub mod decoder;
pub mod frame;
pub mod types;

// Re-export commonly used functions (optimized O(log n) lookups)
pub use database::{database_stats, get_spn_def, get_spns_for_pgn, list_supported_pgns};
pub use decoder::{decode_frame, decode_frame_iter, decode_spn, decode_spn_by_number, decode_spn_full};
pub use frame::{
    build_can_id, build_request_pgn, extract_pgn, extract_source_address, is_valid_j1939_id,
    parse_can_id,
};
pub use types::{DecodedSpn, J1939Id, SpnDataType, SpnDef};
