//! J1939 SPN decoder.
//!
//! Provides utilities for decoding SPN values from CAN frame data.

use crate::database::{get_spn_def, get_spns_for_pgn};
use crate::frame::extract_pgn;
use crate::types::{DecodedSpn, SpnDataType, SpnDef};

/// Extract raw value and check validity in one pass.
/// Returns (raw_value, scaled_value) if valid.
#[inline]
fn extract_and_validate(data: &[u8], spn_def: &SpnDef) -> Option<(u64, f64)> {
    let raw_value = extract_raw_value(data, spn_def)?;

    // Check for "not available" values (all 1s or all 1s minus 1)
    // Compute max_value without overflow for bit_length up to 64
    let max_value = if spn_def.bit_length >= 64 {
        u64::MAX
    } else {
        (1u64 << spn_def.bit_length) - 1
    };

    if raw_value >= max_value.saturating_sub(1) {
        return None;
    }

    let value = (raw_value as f64).mul_add(spn_def.scale, spn_def.offset);
    Some((raw_value, value))
}

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
#[inline]
pub fn decode_spn(data: &[u8], spn_def: &SpnDef) -> Option<f64> {
    extract_and_validate(data, spn_def).map(|(_, value)| value)
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
#[inline]
pub fn decode_spn_full(data: &[u8], spn_def: &SpnDef) -> Option<DecodedSpn> {
    let (raw_value, value) = extract_and_validate(data, spn_def)?;
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
#[inline]
pub fn decode_frame(can_id: u32, data: &[u8]) -> Vec<DecodedSpn> {
    let pgn = extract_pgn(can_id);

    let Some(spn_defs) = get_spns_for_pgn(pgn) else {
        return Vec::new();
    };

    // Pre-allocate with exact capacity, then filter_map to avoid intermediate allocations
    spn_defs
        .iter()
        .filter_map(|spn_def| decode_spn_full(data, spn_def))
        .collect()
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
#[inline]
pub fn decode_spn_by_number(spn: u32, data: &[u8]) -> Option<f64> {
    decode_spn(data, get_spn_def(spn)?)
}

// ============================================================================
// Internal helpers - optimized for minimal branching
// ============================================================================

/// Extract raw value from data bytes based on SPN definition.
/// Uses get_unchecked after bounds check for better codegen.
#[inline]
fn extract_raw_value(data: &[u8], spn_def: &SpnDef) -> Option<u64> {
    let start = spn_def.start_byte as usize;

    // Compute required length based on data type
    let required_len = start + match spn_def.data_type {
        SpnDataType::Uint8 | SpnDataType::Int8 => 1,
        SpnDataType::Uint16 | SpnDataType::Int16 => 2,
        SpnDataType::Uint32 | SpnDataType::Int32 => 4,
    };

    // Single bounds check for all types
    if data.len() < required_len {
        return None;
    }

    // SAFETY: We just verified bounds above
    let val = match spn_def.data_type {
        SpnDataType::Uint8 => {
            let byte = data[start];
            if spn_def.bit_length == 8 && spn_def.start_bit == 0 {
                byte as u64
            } else {
                // Bit field extraction - compute mask without branching
                let mask = (1u8 << spn_def.bit_length).wrapping_sub(1);
                ((byte >> spn_def.start_bit) & mask) as u64
            }
        }
        SpnDataType::Uint16 => {
            // Use array indexing which compiles to efficient load
            u16::from_le_bytes([data[start], data[start + 1]]) as u64
        }
        SpnDataType::Uint32 => {
            u32::from_le_bytes([data[start], data[start + 1], data[start + 2], data[start + 3]])
                as u64
        }
        SpnDataType::Int8 => data[start] as i8 as u64,
        SpnDataType::Int16 => {
            i16::from_le_bytes([data[start], data[start + 1]]) as u64
        }
        SpnDataType::Int32 => {
            i32::from_le_bytes([data[start], data[start + 1], data[start + 2], data[start + 3]])
                as u64
        }
    };

    Some(val)
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
