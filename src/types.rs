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

impl SpnDataType {
    /// Returns the number of bytes required to store this data type.
    #[inline]
    pub const fn byte_size(self) -> usize {
        match self {
            Self::Uint8 | Self::Int8 => 1,
            Self::Uint16 | Self::Int16 => 2,
            Self::Uint32 | Self::Int32 => 4,
        }
    }

    /// Returns true if this is a signed type.
    #[inline]
    pub const fn is_signed(self) -> bool {
        matches!(self, Self::Int8 | Self::Int16 | Self::Int32)
    }
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

    // ========================================================================
    // SpnDataType tests
    // ========================================================================

    #[test]
    fn test_spn_data_type_byte_size() {
        assert_eq!(SpnDataType::Uint8.byte_size(), 1);
        assert_eq!(SpnDataType::Int8.byte_size(), 1);
        assert_eq!(SpnDataType::Uint16.byte_size(), 2);
        assert_eq!(SpnDataType::Int16.byte_size(), 2);
        assert_eq!(SpnDataType::Uint32.byte_size(), 4);
        assert_eq!(SpnDataType::Int32.byte_size(), 4);
    }

    #[test]
    fn test_spn_data_type_is_signed() {
        assert!(!SpnDataType::Uint8.is_signed());
        assert!(!SpnDataType::Uint16.is_signed());
        assert!(!SpnDataType::Uint32.is_signed());
        assert!(SpnDataType::Int8.is_signed());
        assert!(SpnDataType::Int16.is_signed());
        assert!(SpnDataType::Int32.is_signed());
    }

    #[test]
    fn test_spn_data_type_repr() {
        // Verify repr(u8) - each variant should be 1 byte
        assert_eq!(std::mem::size_of::<SpnDataType>(), 1);
    }

    // ========================================================================
    // J1939Id tests
    // ========================================================================

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

    #[test]
    fn test_j1939_id_size() {
        // Should be 8 bytes with repr(C): pgn(4) + priority(1) + sa(1) + da(1) + padding(1)
        assert!(std::mem::size_of::<J1939Id>() <= 8);
    }

    #[test]
    fn test_j1939_id_pdu1_format() {
        // PDU1 format: PF < 240, destination address used
        let id = J1939Id {
            priority: 6,
            pgn: 0xEA00, // PF = 0xEA = 234
            source_address: 0xFE,
            destination_address: 0x00,
        };

        let can_id = id.to_can_id();
        // Should include destination address in PS field
        assert_eq!((can_id >> 8) & 0xFF, 0x00); // PS = DA
        assert_eq!(can_id & 0xFF, 0xFE); // SA
    }

    #[test]
    fn test_j1939_id_pdu2_format() {
        // PDU2 format: PF >= 240, PS is part of PGN
        let id = J1939Id {
            priority: 6,
            pgn: 65262, // ET1: PF=0xFE, PS=0xEE
            source_address: 0x00,
            destination_address: 0xFF, // Ignored for PDU2
        };

        let can_id = id.to_can_id();
        // PS should be from PGN, not destination_address
        assert_eq!((can_id >> 8) & 0xFF, 0xEE); // PS from PGN
    }

    #[test]
    fn test_j1939_id_priority_range() {
        for priority in 0..=7 {
            let id = J1939Id {
                priority,
                pgn: 61444,
                source_address: 0x00,
                destination_address: 0xFF,
            };
            let can_id = id.to_can_id();
            assert_eq!((can_id >> 26) & 0x07, priority as u32);
        }
    }

    #[test]
    fn test_j1939_id_copy() {
        let id1 = J1939Id {
            priority: 3,
            pgn: 61444,
            source_address: 0x00,
            destination_address: 0xFF,
        };
        let id2 = id1; // Copy
        assert_eq!(id1.pgn, id2.pgn);
        assert_eq!(id1.priority, id2.priority);
    }

    // ========================================================================
    // DecodedSpn tests
    // ========================================================================

    #[test]
    fn test_decoded_spn_copy() {
        let spn1 = DecodedSpn {
            value: 100.0,
            raw_value: 100,
            spn: 190,
            name: "test",
            unit: "RPM",
        };
        let spn2 = spn1; // Copy
        assert_eq!(spn1.value, spn2.value);
        assert_eq!(spn1.spn, spn2.spn);
    }

    #[test]
    fn test_decoded_spn_size() {
        // DecodedSpn should be reasonably sized
        // f64(8) + u64(8) + u32(4) + 2*&str(16 each on 64-bit) = 52 bytes
        // With padding it should be around 56 bytes
        assert!(std::mem::size_of::<DecodedSpn>() <= 64);
    }

    // ========================================================================
    // SpnDef tests
    // ========================================================================

    #[test]
    fn test_spn_def_copy() {
        let def1 = SpnDef {
            scale: 1.0,
            offset: 0.0,
            spn: 100,
            pgn: 61444,
            name: "test",
            unit: "C",
            start_byte: 0,
            start_bit: 0,
            bit_length: 8,
            data_type: SpnDataType::Uint8,
        };
        let def2 = def1; // Copy
        assert_eq!(def1.spn, def2.spn);
        assert_eq!(def1.scale, def2.scale);
    }
}
