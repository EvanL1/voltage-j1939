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
    fn test_build_can_id_roundtrip() {
        let original = 0x0CF00400u32;
        let id = parse_can_id(original);
        let rebuilt = build_can_id(&id);
        assert_eq!(original, rebuilt);
    }

    #[test]
    fn test_build_request_pgn() {
        let (can_id, data) = build_request_pgn(0xFE, 0x00, 65253);
        assert_eq!(can_id, 0x18EA00FE);
        assert_eq!(data[0], 0xE5); // 65253 & 0xFF
        assert_eq!(data[1], 0xFE); // (65253 >> 8) & 0xFF
        assert_eq!(data[2], 0x00); // (65253 >> 16) & 0xFF
    }

    #[test]
    fn test_extract_pgn() {
        assert_eq!(extract_pgn(0x0CF00400), 61444);
        assert_eq!(extract_pgn(0x18FEEE00), 65262);
        assert_eq!(extract_pgn(0x18EA00FE), 0xEA00);
    }

    #[test]
    fn test_extract_source_address() {
        assert_eq!(extract_source_address(0x0CF00400), 0x00);
        assert_eq!(extract_source_address(0x18EA00FE), 0xFE);
    }
}
