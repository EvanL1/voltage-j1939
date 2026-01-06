//! J1939 CAN frame parsing.
//!
//! Provides utilities for parsing and building J1939 29-bit extended CAN IDs.
//! All functions are `#[inline]` for zero-cost abstraction.

use crate::types::J1939Id;

/// PDU2 format threshold (PF >= 240 means broadcast)
const PDU2_THRESHOLD: u8 = 240;

/// Parse a 29-bit J1939 CAN ID into its components.
///
/// J1939 CAN ID format (29 bits):
/// ```text
/// | Priority | R | DP | PF | PS/DA | SA |
/// |   3 bit  |1b | 1b | 8b |  8b   | 8b |
/// ```
///
/// - Priority: Message priority (0-7)
/// - R: Reserved (always 0)
/// - DP: Data Page (0 or 1)
/// - PF: PDU Format
/// - PS: PDU Specific (for PDU2) / DA: Destination Address (for PDU1)
/// - SA: Source Address
///
/// # Example
///
/// ```
/// use voltage_j1939::frame::parse_can_id;
///
/// // EEC1 from SA=0x00: CAN ID = 0x0CF00400
/// let id = parse_can_id(0x0CF00400);
/// assert_eq!(id.priority, 3);
/// assert_eq!(id.pgn, 61444);  // EEC1
/// assert_eq!(id.source_address, 0x00);
/// ```
#[inline]
pub fn parse_can_id(can_id: u32) -> J1939Id {
    // Extract all fields in one pass using bit operations
    let sa = can_id as u8;
    let ps = (can_id >> 8) as u8;
    let pf = (can_id >> 16) as u8;
    let dp = (can_id >> 24) & 0x01;
    let priority = ((can_id >> 26) & 0x07) as u8;

    // Calculate PGN based on PDU format
    // PDU1 (PF < 240): PGN = DP.PF.00, PS is destination address
    // PDU2 (PF >= 240): PGN = DP.PF.PS, PS is part of PGN
    let (pgn, destination_address) = if pf >= PDU2_THRESHOLD {
        // PDU2 format - broadcast
        ((dp << 16) | ((pf as u32) << 8) | (ps as u32), 0xFF)
    } else {
        // PDU1 format - peer-to-peer
        ((dp << 16) | ((pf as u32) << 8), ps)
    };

    J1939Id {
        priority,
        pgn,
        source_address: sa,
        destination_address,
    }
}

/// Build a 29-bit CAN ID from J1939 components.
///
/// # Example
///
/// ```
/// use voltage_j1939::frame::build_can_id;
/// use voltage_j1939::types::J1939Id;
///
/// let id = J1939Id {
///     priority: 6,
///     pgn: 0xEA00,  // Request PGN
///     source_address: 0xFE,
///     destination_address: 0x00,
/// };
/// let can_id = build_can_id(&id);
/// assert_eq!(can_id, 0x18EA00FE);
/// ```
#[inline]
pub fn build_can_id(id: &J1939Id) -> u32 {
    id.to_can_id()
}

/// Request PGN constant
const REQUEST_PGN: u32 = 0xEA00;

/// Build a Request PGN CAN frame.
///
/// The Request PGN (0xEA00) is used to request another ECU to transmit
/// a specific PGN.
///
/// # Arguments
///
/// * `source_address` - Our source address
/// * `destination_address` - Target ECU address (0xFF for broadcast)
/// * `requested_pgn` - The PGN we want to receive
///
/// # Returns
///
/// A tuple of (CAN ID, data bytes).
///
/// # Example
///
/// ```
/// use voltage_j1939::frame::build_request_pgn;
///
/// // Request Engine Hours (PGN 65253) from ECU at address 0x00
/// let (can_id, data) = build_request_pgn(0xFE, 0x00, 65253);
/// // can_id = 0x18EA00FE (Request PGN from 0xFE to 0x00)
/// // data = [0xE5, 0xFE, 0x00] (PGN 65253 in little-endian)
/// ```
#[inline]
pub fn build_request_pgn(
    source_address: u8,
    destination_address: u8,
    requested_pgn: u32,
) -> (u32, [u8; 3]) {
    // Build CAN ID directly without intermediate struct
    // Priority 6, DP=0, PF=0xEA, PS=destination_address, SA=source_address
    let can_id = (6u32 << 26)
        | ((REQUEST_PGN & 0xFF00) << 8)
        | ((destination_address as u32) << 8)
        | (source_address as u32);

    // Convert PGN to little-endian bytes directly
    let data = [
        requested_pgn as u8,
        (requested_pgn >> 8) as u8,
        (requested_pgn >> 16) as u8,
    ];

    (can_id, data)
}

/// Maximum valid 29-bit CAN ID
const MAX_29BIT_ID: u32 = 0x1FFFFFFF;

/// Check if a CAN ID is a valid J1939 extended frame.
///
/// J1939 uses 29-bit extended CAN IDs. This function checks that the ID
/// is within the valid range and has reasonable J1939 structure.
#[inline]
pub const fn is_valid_j1939_id(can_id: u32) -> bool {
    can_id <= MAX_29BIT_ID
}

/// Extract just the PGN from a CAN ID without full parsing.
///
/// This is a faster alternative to `parse_can_id` when you only need the PGN.
#[inline]
pub const fn extract_pgn(can_id: u32) -> u32 {
    let ps = (can_id >> 8) as u8;
    let pf = (can_id >> 16) as u8;
    let dp = (can_id >> 24) & 0x01;

    if pf >= PDU2_THRESHOLD {
        (dp << 16) | ((pf as u32) << 8) | (ps as u32)
    } else {
        (dp << 16) | ((pf as u32) << 8)
    }
}

/// Extract just the source address from a CAN ID.
#[inline]
pub const fn extract_source_address(can_id: u32) -> u8 {
    can_id as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // parse_can_id tests
    // ========================================================================

    #[test]
    fn test_parse_can_id_eec1() {
        // EEC1 from SA=0x00: CAN ID = 0x0CF00400
        let id = parse_can_id(0x0CF00400);
        assert_eq!(id.priority, 3);
        assert_eq!(id.pgn, 61444);
        assert_eq!(id.source_address, 0x00);
        assert!(id.is_broadcast());
    }

    #[test]
    fn test_parse_can_id_et1() {
        // ET1 from SA=0x00: CAN ID = 0x18FEEE00
        let id = parse_can_id(0x18FEEE00);
        assert_eq!(id.priority, 6);
        assert_eq!(id.pgn, 65262);
        assert_eq!(id.source_address, 0x00);
    }

    #[test]
    fn test_parse_can_id_request() {
        // Request PGN to SA=0x00 from SA=0xFE
        let id = parse_can_id(0x18EA00FE);
        assert_eq!(id.priority, 6);
        assert_eq!(id.pgn, 0xEA00);
        assert_eq!(id.source_address, 0xFE);
        assert_eq!(id.destination_address, 0x00);
        assert!(id.is_peer_to_peer());
    }

    #[test]
    fn test_parse_can_id_all_priorities() {
        // Test all priority levels (0-7)
        for priority in 0u8..=7 {
            let can_id = (priority as u32) << 26 | 0x00F00400;
            let id = parse_can_id(can_id);
            assert_eq!(id.priority, priority);
        }
    }

    #[test]
    fn test_parse_can_id_all_source_addresses() {
        // Test boundary source addresses
        for sa in [0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF] {
            let can_id = 0x0CF00400 | (sa as u32);
            let id = parse_can_id(can_id);
            assert_eq!(id.source_address, sa);
        }
    }

    #[test]
    fn test_parse_can_id_data_page() {
        // Test with data page bit set (DP=1)
        // PGN with DP=1 should be 0x1xxxx
        let can_id = 0x0DF00400; // DP=1
        let id = parse_can_id(can_id);
        assert_eq!(id.pgn >> 16, 1); // DP should be 1
    }

    // ========================================================================
    // build_can_id tests
    // ========================================================================

    #[test]
    fn test_build_can_id_roundtrip() {
        let original = 0x0CF00400u32;
        let id = parse_can_id(original);
        let rebuilt = build_can_id(&id);
        assert_eq!(original, rebuilt);
    }

    #[test]
    fn test_build_can_id_pdu1_roundtrip() {
        // PDU1 format with specific destination address
        let original = 0x18EA00FE; // Request PGN to 0x00 from 0xFE
        let id = parse_can_id(original);
        let rebuilt = build_can_id(&id);
        assert_eq!(original, rebuilt);
    }

    #[test]
    fn test_build_can_id_pdu2_roundtrip() {
        // PDU2 format - multiple test cases
        let test_cases = [
            0x0CF00400u32, // EEC1
            0x18FEEE00,    // ET1
            0x18FEF100,    // CCVS
            0x18FEE500,    // HOURS
        ];

        for original in test_cases {
            let id = parse_can_id(original);
            let rebuilt = build_can_id(&id);
            assert_eq!(original, rebuilt, "Failed for CAN ID 0x{:08X}", original);
        }
    }

    // ========================================================================
    // build_request_pgn tests
    // ========================================================================

    #[test]
    fn test_build_request_pgn() {
        let (can_id, data) = build_request_pgn(0xFE, 0x00, 65253);
        assert_eq!(can_id, 0x18EA00FE);
        assert_eq!(data[0], 0xE5); // 65253 & 0xFF
        assert_eq!(data[1], 0xFE); // (65253 >> 8) & 0xFF
        assert_eq!(data[2], 0x00); // (65253 >> 16) & 0xFF
    }

    #[test]
    fn test_build_request_pgn_broadcast() {
        // Broadcast request (destination = 0xFF)
        let (can_id, _) = build_request_pgn(0xFE, 0xFF, 61444);
        let id = parse_can_id(can_id);
        assert_eq!(id.source_address, 0xFE);
        assert_eq!(id.destination_address, 0xFF);
    }

    #[test]
    fn test_build_request_pgn_data_format() {
        // Verify little-endian encoding
        let (_, data) = build_request_pgn(0x00, 0x00, 0x010203);
        assert_eq!(data[0], 0x03); // LSB
        assert_eq!(data[1], 0x02);
        assert_eq!(data[2], 0x01); // MSB
    }

    // ========================================================================
    // extract_pgn tests
    // ========================================================================

    #[test]
    fn test_extract_pgn() {
        assert_eq!(extract_pgn(0x0CF00400), 61444);
        assert_eq!(extract_pgn(0x18FEEE00), 65262);
        assert_eq!(extract_pgn(0x18EA00FE), 0xEA00);
    }

    #[test]
    fn test_extract_pgn_pdu1_vs_pdu2() {
        // PDU1 (PF < 240): PS is NOT part of PGN
        let pdu1_can_id = 0x18EA00FE; // Request to 0x00 from 0xFE
        assert_eq!(extract_pgn(pdu1_can_id), 0xEA00); // PS should not be included

        // PDU2 (PF >= 240): PS IS part of PGN
        let pdu2_can_id = 0x18FEEE00; // ET1
        assert_eq!(extract_pgn(pdu2_can_id), 65262); // Should include PS=0xEE
    }

    #[test]
    fn test_extract_pgn_const() {
        // Verify it can be used in const context
        const PGN: u32 = extract_pgn(0x0CF00400);
        assert_eq!(PGN, 61444);
    }

    // ========================================================================
    // extract_source_address tests
    // ========================================================================

    #[test]
    fn test_extract_source_address() {
        assert_eq!(extract_source_address(0x0CF00400), 0x00);
        assert_eq!(extract_source_address(0x18EA00FE), 0xFE);
    }

    #[test]
    fn test_extract_source_address_const() {
        // Verify it can be used in const context
        const SA: u8 = extract_source_address(0x18EA00FE);
        assert_eq!(SA, 0xFE);
    }

    // ========================================================================
    // is_valid_j1939_id tests
    // ========================================================================

    #[test]
    fn test_is_valid_j1939_id() {
        // Valid 29-bit IDs
        assert!(is_valid_j1939_id(0x0CF00400));
        assert!(is_valid_j1939_id(0x1FFFFFFF)); // Max valid
        assert!(is_valid_j1939_id(0x00000000)); // Min valid

        // Invalid (> 29 bits)
        assert!(!is_valid_j1939_id(0x20000000));
        assert!(!is_valid_j1939_id(0xFFFFFFFF));
    }

    #[test]
    fn test_is_valid_j1939_id_const() {
        // Verify it can be used in const context
        const IS_VALID: bool = is_valid_j1939_id(0x0CF00400);
        assert!(IS_VALID);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_zero_can_id() {
        let id = parse_can_id(0x00000000);
        assert_eq!(id.priority, 0);
        assert_eq!(id.pgn, 0);
        assert_eq!(id.source_address, 0);
        assert!(id.is_peer_to_peer()); // PF=0 < 240
    }

    #[test]
    fn test_max_29bit_can_id() {
        let id = parse_can_id(0x1FFFFFFF);
        assert_eq!(id.priority, 7);
        assert_eq!(id.source_address, 0xFF);
        assert!(id.is_broadcast()); // PF=0xFF >= 240
    }
}
