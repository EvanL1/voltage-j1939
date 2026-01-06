//! J1939 SPN decoder.
//!
//! Provides utilities for decoding SPN values from CAN frame data.

use crate::database::{get_spn_def, get_spns_for_pgn};
use crate::frame::extract_pgn;
use crate::types::{DecodedSpn, SpnDataType, SpnDef};

/// Precomputed "not available" thresholds for each bit length (0-64)
/// Value at index N = (2^N - 3), the threshold above which a value is special.
/// J1939 reserves the last two values: (2^N - 2) = error, (2^N - 1) = not available.
/// For N=0 or N=1, we use 0 (no valid values for degenerate cases).
/// For N>=64, saturates to u64::MAX-2.
const NOT_AVAILABLE_THRESHOLD: [u64; 65] = {
    let mut table = [0u64; 65];
    // For 0 and 1 bit, threshold is 0 (special handling)
    let mut i = 2usize;
    while i < 64 {
        table[i] = (1u64 << i) - 3;
        i += 1;
    }
    table[64] = u64::MAX - 2;
    table
};

/// Extract raw value and check validity in one pass.
/// Returns (raw_value, scaled_value) if valid.
#[inline]
fn extract_and_validate(data: &[u8], spn_def: &SpnDef) -> Option<(u64, f64)> {
    let raw_value = extract_raw_value(data, spn_def)?;

    // Check for "not available" values using precomputed lookup table
    // bit_length is guaranteed to be <= 64 based on SpnDataType
    let threshold = NOT_AVAILABLE_THRESHOLD[spn_def.bit_length as usize];
    if raw_value > threshold {
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

/// Decode all known SPNs from a CAN frame (zero-allocation iterator).
///
/// This is the preferred method for performance-critical code as it
/// avoids heap allocation entirely.
///
/// # Arguments
///
/// * `can_id` - The 29-bit extended CAN ID
/// * `data` - The CAN frame data (up to 8 bytes)
///
/// # Returns
///
/// An iterator over decoded SPNs. Empty iterator if PGN is not recognized.
///
/// # Example
///
/// ```
/// use voltage_j1939::decoder::decode_frame_iter;
///
/// let can_id = 0x0CF00400u32;
/// let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];
///
/// for spn in decode_frame_iter(can_id, &data) {
///     println!("{}: {} {}", spn.name, spn.value, spn.unit);
/// }
/// ```
#[inline]
pub fn decode_frame_iter(
    can_id: u32,
    data: &[u8],
) -> impl Iterator<Item = DecodedSpn> + '_ {
    let pgn = extract_pgn(can_id);
    get_spns_for_pgn(pgn)
        .into_iter()
        .flatten()
        .filter_map(move |spn_def| decode_spn_full(data, spn_def))
}

/// Decode all known SPNs from a CAN frame.
///
/// For zero-allocation iteration, use [`decode_frame_iter`] instead.
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
    decode_frame_iter(can_id, data).collect()
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
/// Uses unsafe get_unchecked after bounds check for optimal codegen.
#[inline]
fn extract_raw_value(data: &[u8], spn_def: &SpnDef) -> Option<u64> {
    let start = spn_def.start_byte as usize;
    let required_len = start + spn_def.data_type.byte_size();

    // Single bounds check - enables optimizer to remove bounds checks below
    if data.len() < required_len {
        return None;
    }

    // SAFETY: We verified data.len() >= start + byte_size above
    let val = unsafe {
        match spn_def.data_type {
            SpnDataType::Uint8 => {
                let byte = *data.get_unchecked(start);
                if spn_def.bit_length == 8 && spn_def.start_bit == 0 {
                    byte as u64
                } else {
                    // Bit field extraction
                    let mask = (1u8 << spn_def.bit_length).wrapping_sub(1);
                    ((byte >> spn_def.start_bit) & mask) as u64
                }
            }
            SpnDataType::Uint16 => {
                let ptr = data.as_ptr().add(start) as *const [u8; 2];
                u16::from_le_bytes(*ptr) as u64
            }
            SpnDataType::Uint32 => {
                let ptr = data.as_ptr().add(start) as *const [u8; 4];
                u32::from_le_bytes(*ptr) as u64
            }
            SpnDataType::Int8 => *data.get_unchecked(start) as i8 as u64,
            SpnDataType::Int16 => {
                let ptr = data.as_ptr().add(start) as *const [u8; 2];
                i16::from_le_bytes(*ptr) as u64
            }
            SpnDataType::Int32 => {
                let ptr = data.as_ptr().add(start) as *const [u8; 4];
                i32::from_le_bytes(*ptr) as u64
            }
        }
    };

    Some(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Basic decoding tests
    // ========================================================================

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

    // ========================================================================
    // Iterator version tests
    // ========================================================================

    #[test]
    fn test_decode_frame_iter() {
        let can_id = 0x0CF00400; // EEC1
        let data = [0x00, 0x00, 0x00, 0x20, 0x4E, 0x00, 0x00, 0x00];

        // Count decoded SPNs using iterator
        let count = decode_frame_iter(can_id, &data).count();
        assert!(count > 0);

        // Find specific SPN using iterator
        let engine_speed = decode_frame_iter(can_id, &data)
            .find(|d| d.spn == 190);
        assert!(engine_speed.is_some());
        assert_eq!(engine_speed.unwrap().value, 2500.0);
    }

    #[test]
    fn test_decode_frame_iter_empty() {
        // Unknown PGN should return empty iterator
        let can_id = 0x00000000;
        let data = [0xFF; 8];
        assert_eq!(decode_frame_iter(can_id, &data).count(), 0);
    }

    #[test]
    fn test_decode_frame_iter_consistency() {
        // Iterator and Vec versions should produce same results
        let can_id = 0x0CF00400;
        let data = [0x05, 0x10, 0x20, 0x20, 0x4E, 0x00, 0x30, 0x40];

        let vec_result: Vec<_> = decode_frame(can_id, &data);
        let iter_result: Vec<_> = decode_frame_iter(can_id, &data).collect();

        assert_eq!(vec_result.len(), iter_result.len());
        for (v, i) in vec_result.iter().zip(iter_result.iter()) {
            assert_eq!(v.spn, i.spn);
            assert_eq!(v.value, i.value);
            assert_eq!(v.raw_value, i.raw_value);
        }
    }

    // ========================================================================
    // Boundary condition tests
    // ========================================================================

    #[test]
    fn test_decode_data_too_short() {
        // SPN 190 needs bytes 3-4 (16-bit at byte 3)
        let spn_def = get_spn_def(190).unwrap();

        // Only 4 bytes, needs at least 5
        let data = [0, 0, 0, 0x20];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());

        // Exactly enough bytes
        let data = [0, 0, 0, 0x20, 0x4E];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(2500.0));
    }

    #[test]
    fn test_decode_empty_data() {
        let spn_def = get_spn_def(110).unwrap();
        let data: [u8; 0] = [];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    #[test]
    fn test_decode_not_available_16bit() {
        // 0xFFFF means "not available" for 16-bit values
        let spn_def = get_spn_def(190).unwrap(); // Engine speed (16-bit at byte 3)
        let data = [0, 0, 0, 0xFF, 0xFF, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());

        // 0xFFFE also means "not available" (error indicator)
        let data = [0, 0, 0, 0xFE, 0xFF, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    #[test]
    fn test_decode_32bit_values() {
        // SPN 247 = Engine Total Hours (32-bit at byte 0)
        let spn_def = get_spn_def(247).unwrap();

        // 72000 raw * 0.05 = 3600 hours
        // 72000 = 0x00011940 -> little-endian [0x40, 0x19, 0x01, 0x00]
        let data = [0x40, 0x19, 0x01, 0x00, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(3600.0));
    }

    #[test]
    fn test_decode_32bit_not_available() {
        // SPN 247 = Engine Total Hours (32-bit)
        let spn_def = get_spn_def(247).unwrap();

        // 0xFFFFFFFF means "not available"
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    #[test]
    fn test_decode_zero_value() {
        let spn_def = get_spn_def(110).unwrap();
        let data = [0u8, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        // 0 * 1.0 + (-40) = -40
        assert_eq!(value, Some(-40.0));
    }

    #[test]
    fn test_decode_negative_offset() {
        // SPN 110 has offset -40
        let spn_def = get_spn_def(110).unwrap();

        // Raw 40 -> 40 * 1.0 + (-40) = 0°C
        let data = [40u8, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(0.0));
    }

    #[test]
    fn test_decode_full_returns_metadata() {
        let spn_def = get_spn_def(190).unwrap();
        let data = [0, 0, 0, 0x20, 0x4E, 0, 0, 0];

        let decoded = decode_spn_full(&data, spn_def).unwrap();
        assert_eq!(decoded.spn, 190);
        assert_eq!(decoded.name, "engine_speed");
        assert_eq!(decoded.unit, "RPM");
        assert_eq!(decoded.raw_value, 20000);
        assert_eq!(decoded.value, 2500.0);
    }

    // ========================================================================
    // Bit field tests
    // ========================================================================

    #[test]
    fn test_bit_field_with_offset() {
        // SPN 559 = Accelerator Pedal Kickdown (2 bits at byte 0, bit 2)
        // For 2-bit fields: 0,1 = valid, 2 = error, 3 = n/a
        let spn_def = get_spn_def(559).unwrap();

        // Value 0b01 at bit position 2 = 0b0100 = 0x04
        let data = [0x04, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(1.0));

        // Value 0b00 at bit position 2 = 0b0000 = 0x00
        let data = [0x00, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(0.0));

        // Value 0b11 at bit position 2 = 0b1100 = 0x0C (not available)
        let data = [0x0C, 0, 0, 0, 0, 0, 0, 0];
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    #[test]
    fn test_bit_field_max_valid() {
        // SPN 899 = Engine Torque Mode (4 bits)
        // Max valid value for 4 bits is 13 (14 and 15 are "not available")
        let spn_def = get_spn_def(899).unwrap();

        let data = [0x0D, 0, 0, 0, 0, 0, 0, 0]; // 13
        let value = decode_spn(&data, spn_def);
        assert_eq!(value, Some(13.0));

        let data = [0x0E, 0, 0, 0, 0, 0, 0, 0]; // 14 - not available
        let value = decode_spn(&data, spn_def);
        assert!(value.is_none());
    }

    // ========================================================================
    // Multiple PGN tests
    // ========================================================================

    #[test]
    fn test_decode_multiple_pgns() {
        // ET1 (PGN 65262) - Engine Temperature
        let et1_can_id = 0x18FEEE00;
        let et1_data = [130u8, 50, 0, 0, 0, 0, 40, 0]; // coolant=90°C, fuel=10°C

        let decoded = decode_frame(et1_can_id, &et1_data);

        // Find coolant temperature
        let coolant = decoded.iter().find(|d| d.spn == 110);
        assert!(coolant.is_some());
        assert_eq!(coolant.unwrap().value, 90.0);

        // Find fuel temperature
        let fuel = decoded.iter().find(|d| d.spn == 174);
        assert!(fuel.is_some());
        assert_eq!(fuel.unwrap().value, 10.0);
    }

    #[test]
    fn test_decode_unknown_pgn() {
        // Unknown PGN should return empty
        let can_id = 0x18FF0000; // Proprietary PGN
        let data = [0xFF; 8];

        let decoded = decode_frame(can_id, &data);
        assert!(decoded.is_empty());
    }

    // ========================================================================
    // NOT_AVAILABLE_THRESHOLD table tests
    // ========================================================================

    #[test]
    fn test_not_available_threshold_table() {
        // Verify the precomputed table is correct
        // J1939 reserves last two values: (2^N - 2) = error, (2^N - 1) = n/a
        // So threshold = (2^N - 3), values > threshold are special
        assert_eq!(NOT_AVAILABLE_THRESHOLD[0], 0);
        assert_eq!(NOT_AVAILABLE_THRESHOLD[1], 0);
        assert_eq!(NOT_AVAILABLE_THRESHOLD[2], 1); // 2^2 - 3 = 1
        assert_eq!(NOT_AVAILABLE_THRESHOLD[4], 13); // 2^4 - 3 = 13
        assert_eq!(NOT_AVAILABLE_THRESHOLD[8], 253); // 2^8 - 3 = 253
        assert_eq!(NOT_AVAILABLE_THRESHOLD[16], 65533); // 2^16 - 3
        assert_eq!(NOT_AVAILABLE_THRESHOLD[32], 0xFFFFFFFD); // 2^32 - 3
    }
}
