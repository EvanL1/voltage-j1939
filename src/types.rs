//! Core types for J1939 protocol.
//!
//! All types are designed for zero-cost abstractions:
//! - `Copy` where possible to avoid heap allocations
//! - `#[repr(u8)]` for enums to minimize size
//! - Fields ordered by size to minimize padding

/// Data type for SPN values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpnDataType {
    /// Unsigned 8-bit integer.
    Uint8 = 0,
    /// Unsigned 16-bit integer.
    Uint16 = 1,
    /// Unsigned 32-bit integer.
    Uint32 = 2,
    /// Signed 8-bit integer.
    Int8 = 3,
    /// Signed 16-bit integer.
    Int16 = 4,
    /// Signed 32-bit integer.
    Int32 = 5,
}

/// SPN (Suspect Parameter Number) definition.
///
/// Contains all metadata needed to decode a specific parameter from a J1939 PGN.
/// This struct is `Copy` since it only contains primitive types and static references.
#[derive(Debug, Clone, Copy)]
pub struct SpnDef {
    /// Scale factor to convert raw value to engineering units.
    pub scale: f64,
    /// Offset to apply after scaling.
    pub offset: f64,
    /// SPN number (unique identifier per SAE J1939 standard).
    pub spn: u32,
    /// PGN that contains this SPN.
    pub pgn: u32,
    /// Human-readable name.
    pub name: &'static str,
    /// Engineering unit string.
    pub unit: &'static str,
    /// Starting byte position in the PGN data (0-indexed).
    pub start_byte: u8,
    /// Starting bit position within the byte (0-indexed, LSB first).
    pub start_bit: u8,
    /// Number of bits used for this value.
    pub bit_length: u8,
    /// Data type of the raw value.
    pub data_type: SpnDataType,
}

/// Decoded SPN value with metadata.
///
/// This struct is `Copy` for efficient pass-by-value semantics.
#[derive(Debug, Clone, Copy)]
pub struct DecodedSpn {
    /// Decoded value in engineering units.
    pub value: f64,
    /// Raw value before scaling.
    pub raw_value: u64,
    /// SPN number.
    pub spn: u32,
    /// Parameter name.
    pub name: &'static str,
    /// Engineering unit.
    pub unit: &'static str,
}

/// PDU2 format threshold (PF >= 240 means broadcast)
const PDU2_THRESHOLD: u32 = 240;

/// J1939 CAN ID components.
///
/// A 29-bit extended CAN ID broken down into J1939 fields.
/// Fields ordered to minimize padding (8 bytes total).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct J1939Id {
    /// Parameter Group Number.
    pub pgn: u32,
    /// Message priority (0-7, lower is higher priority).
    pub priority: u8,
    /// Source Address (SA) of the transmitting ECU.
    pub source_address: u8,
    /// Destination Address (for PDU1 format, PS field).
    pub destination_address: u8,
}

impl J1939Id {
    /// Check if this is a broadcast message (PDU2 format).
    #[inline]
    pub const fn is_broadcast(&self) -> bool {
        // PDU2 format: PF >= 240
        (self.pgn >> 8) & 0xFF >= PDU2_THRESHOLD
    }

    /// Check if this is a peer-to-peer message (PDU1 format).
    #[inline]
    pub const fn is_peer_to_peer(&self) -> bool {
        !self.is_broadcast()
    }

    /// Build a 29-bit CAN ID from J1939 components.
    #[inline]
    pub const fn to_can_id(&self) -> u32 {
        let dp = (self.pgn >> 16) & 0x01;
        let pf = (self.pgn >> 8) & 0xFF;
        let ps = if pf >= PDU2_THRESHOLD {
            self.pgn & 0xFF
        } else {
            self.destination_address as u32
        };

        ((self.priority as u32) << 26)
            | (dp << 24)
            | (pf << 16)
            | (ps << 8)
            | (self.source_address as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_j1939_id_roundtrip() {
        let id = J1939Id {
            priority: 3,
            pgn: 61444, // EEC1
            source_address: 0x00,
            destination_address: 0xFF,
        };

        let can_id = id.to_can_id();
        assert_eq!(can_id, 0x0CF00400);
    }

    #[test]
    fn test_broadcast_detection() {
        // EEC1 (PGN 61444, PF=240) is broadcast
        let id = J1939Id {
            priority: 3,
            pgn: 61444,
            source_address: 0x00,
            destination_address: 0xFF,
        };
        assert!(id.is_broadcast());

        // Request PGN (0xEA00, PF=234) is peer-to-peer
        let id = J1939Id {
            priority: 6,
            pgn: 0xEA00,
            source_address: 0xFE,
            destination_address: 0x00,
        };
        assert!(id.is_peer_to_peer());
    }
}
