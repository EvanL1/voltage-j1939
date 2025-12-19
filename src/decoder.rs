//! J1939 SPN decoder.
//!
//! Provides utilities for decoding SPN values from CAN frame data.

use crate::database::{get_spn_def, get_spns_for_pgn};
use crate::frame::{extract_pgn, extract_source_address};
use crate::types::{DecodedSpn, SpnDataType, SpnDef};

/// Decode a single SPN from CAN data bytes.
///
/// Returns `None` if the data is too short or the value indicates "not available".
///
/// # Example
///
/// ```
/// use voltage_j1939::decoder::decode_spn;
/// use voltage_j1939::database::get_spn_def;
///
/// // Decode coolant temperature (SPN 110)
/// let spn_def = get_spn_def(110).unwrap();
/// let data = [130u8, 0, 0, 0, 0, 0, 0, 0]; // Raw value 130
///
/// if let Some(value) = decode_spn(&data, spn_def) {
///     // value = 130 * 1.0 + (-40) = 90°C
///     assert_eq!(value, 90.0);
/// }
/// ```
pub fn decode_spn(data: &[u8], spn_def: &SpnDef) -> Option<f64> {
    if data.len() <= spn_def.start_byte as usize {
        return None;
    }

    let raw_value = extract_raw_value(data, spn_def)?;

    // Check for "not available" values (all 1s)
    let max_value = (1u64 << spn_def.bit_length) - 1;
    if raw_value >= max_value - 1 {
        return None;
    }

    // Apply scale and offset
    let value = (raw_value as f64) * spn_def.scale + spn_def.offset;
    Some(value)
}

/// Decode a single SPN and return full decoded information.
///
/// # Example
///
/// ```
/// use voltage_j1939::decoder::decode_spn_full;
/// use voltage_j1939::database::get_spn_def;
///
/// let spn_def = get_spn_def(190).unwrap(); // Engine speed
/// let data = [0, 0, 0, 0x20, 0x4E, 0, 0, 0]; // 20000 raw = 2500 RPM
///
/// if let Some(decoded) = decode_spn_full(&data, spn_def) {
///     println!("SPN {}: {} = {} {}", decoded.spn, decoded.name, decoded.value, decoded.unit);
/// }
/// ```
pub fn decode_spn_full(data: &[u8], spn_def: &SpnDef) -> Option<DecodedSpn> {
    if data.len() <= spn_def.start_byte as usize {
        return None;
    }

    let raw_value = extract_raw_value(data, spn_def)?;

    // Check for "not available" values
    let max_value = (1u64 << spn_def.bit_length) - 1;
    if raw_value >= max_value - 1 {
        return None;
    }

    let value = (raw_value as f64) * spn_def.scale + spn_def.offset;

    Some(DecodedSpn {
        spn: spn_def.spn,
        name: spn_def.name,
        value,
        unit: spn_def.unit,
        raw_value,
    })
}

/// Decode all known SPNs from a CAN frame.
///
/// # Arguments
///
/// * `can_id` - The 29-bit extended CAN ID
/// * `data` - The CAN frame data (up to 8 bytes)
///
/// # Returns
///
/// A vector of decoded SPNs. Empty if PGN is not recognized.
///
/// # Example
///
/// ```
/// use voltage_j1939::decoder::decode_frame;
///
/// // EEC1 frame from SA=0x00
/// let can_id = 0x0CF00400u32;
/// let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];
///
/// let decoded = decode_frame(can_id, &data);
/// for spn in decoded {
///     println!("{}: {} {}", spn.name, spn.value, spn.unit);
/// }
/// ```
pub fn decode_frame(can_id: u32, data: &[u8]) -> Vec<DecodedSpn> {
    let pgn = extract_pgn(can_id);

    let Some(spn_defs) = get_spns_for_pgn(pgn) else {
        return Vec::new();
    };

    let mut results = Vec::with_capacity(spn_defs.len());

    for spn_def in spn_defs {
        if let Some(decoded) = decode_spn_full(data, spn_def) {
            results.push(decoded);
        }
    }

    results
}

/// Decode a specific SPN by number from a CAN frame.
///
/// # Arguments
///
/// * `spn` - The SPN number to decode
/// * `data` - The CAN frame data
///
/// # Returns
///
/// The decoded value, or `None` if the SPN is not found or data is invalid.
pub fn decode_spn_by_number(spn: u32, data: &[u8]) -> Option<f64> {
    let spn_def = get_spn_def(spn)?;
    decode_spn(data, spn_def)
}

/// Get the source address from a CAN ID.
pub fn get_source_address(can_id: u32) -> u8 {
    extract_source_address(can_id)
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Extract raw value from data bytes based on SPN definition.
fn extract_raw_value(data: &[u8], spn_def: &SpnDef) -> Option<u64> {
    let start = spn_def.start_byte as usize;

    match spn_def.data_type {
        SpnDataType::Uint8 => {
            if start >= data.len() {
                return None;
            }
            if spn_def.bit_length == 8 && spn_def.start_bit == 0 {
                Some(data[start] as u64)
            } else {
                // Bit field extraction
                let byte = data[start];
                let mask = (1u8 << spn_def.bit_length) - 1;
                Some(((byte >> spn_def.start_bit) & mask) as u64)
            }
        }
        SpnDataType::Uint16 => {
            if start + 1 >= data.len() {
                return None;
            }
            Some(u16::from_le_bytes([data[start], data[start + 1]]) as u64)
        }
        SpnDataType::Uint32 => {
            if start + 3 >= data.len() {
                return None;
            }
            Some(u32::from_le_bytes([
                data[start],
                data[start + 1],
                data[start + 2],
                data[start + 3],
            ]) as u64)
        }
        SpnDataType::Int8 => {
            if start >= data.len() {
                return None;
            }
            Some(data[start] as i8 as i64 as u64)
        }
        SpnDataType::Int16 => {
            if start + 1 >= data.len() {
                return None;
            }
            let val = i16::from_le_bytes([data[start], data[start + 1]]);
            Some(val as i64 as u64)
        }
        SpnDataType::Int32 => {
            if start + 3 >= data.len() {
                return None;
            }
            let val = i32::from_le_bytes([
                data[start],
                data[start + 1],
                data[start + 2],
                data[start + 3],
            ]);
            Some(val as i64 as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_coolant_temp() {
        // SPN 110 = Engine Coolant Temperature
        // Raw 130 -> 130 * 1.0 + (-40) = 90°C
        let spn_def = get_spn_def(110).unwrap();
        let data = [130u8, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(90.0));
    }

    #[test]
    fn test_decode_engine_speed() {
        // SPN 190 = Engine Speed
        // Raw 20000 (0x4E20) -> 20000 * 0.125 = 2500 RPM
        let spn_def = get_spn_def(190).unwrap();
        let data = [0, 0, 0, 0x20, 0x4E, 0, 0, 0]; // Little-endian at byte 3
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(2500.0));
    }

    #[test]
    fn test_decode_not_available() {
        // 0xFF (all 1s) means "not available" for 8-bit values
        let spn_def = get_spn_def(110).unwrap();
        let data = [0xFF, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    #[test]
    fn test_decode_frame_eec1() {
        // EEC1 (PGN 61444) with some test data
        let can_id = 0x0CF00400;
        let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];

        let decoded = decode_frame(can_id, &data);
        assert!(!decoded.is_empty());

        // Should contain engine speed (SPN 190)
        let engine_speed = decoded.iter().find(|d| d.spn == 190);
        assert!(engine_speed.is_some());
        assert_eq!(engine_speed.unwrap().value, 2500.0);
    }

    #[test]
    fn test_decode_spn_by_number() {
        let data = [130u8, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn_by_number(110, &data);
        assert_eq!(value, Some(90.0));

        // Unknown SPN
        let value = decode_spn_by_number(99999, &data);
        assert!(value.is_none());
    }

    #[test]
    fn test_bit_field_extraction() {
        // SPN 899 = Engine Torque Mode (4 bits at byte 0, bit 0)
        let spn_def = get_spn_def(899).unwrap();

        // Value 5 in lower nibble
        let data = [0x05, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(5.0));

        // Value 10 in lower nibble
        let data = [0x0A, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(10.0));
    }
}
