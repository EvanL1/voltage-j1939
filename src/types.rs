//! Core types for J1939 protocol.

/// Data type for SPN values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpnDataType {
    /// Unsigned 8-bit integer.
    Uint8,
    /// Unsigned 16-bit integer.
    Uint16,
    /// Unsigned 32-bit integer.
    Uint32,
    /// Signed 8-bit integer.
    Int8,
    /// Signed 16-bit integer.
    Int16,
    /// Signed 32-bit integer.
    Int32,
}

/// SPN (Suspect Parameter Number) definition.
///
/// Contains all metadata needed to decode a specific parameter from a J1939 PGN.
#[derive(Debug, Clone)]
pub struct SpnDef {
    /// SPN number (unique identifier per SAE J1939 standard).
    pub spn: u32,
    /// Human-readable name.
    pub name: &'static str,
    /// PGN that contains this SPN.
    pub pgn: u32,
    /// Starting byte position in the PGN data (0-indexed).
    pub start_byte: u8,
    /// Starting bit position within the byte (0-indexed, LSB first).
    pub start_bit: u8,
    /// Number of bits used for this value.
    pub bit_length: u8,
    /// Scale factor to convert raw value to engineering units.
    pub scale: f64,
    /// Offset to apply after scaling.
    pub offset: f64,
    /// Engineering unit string.
    pub unit: &'static str,
    /// Data type of the raw value.
    pub data_type: SpnDataType,
}

/// Decoded SPN value with metadata.
#[derive(Debug, Clone)]
pub struct DecodedSpn {
    /// SPN number.
    pub spn: u32,
    /// Parameter name.
    pub name: &'static str,
    /// Decoded value in engineering units.
    pub value: f64,
    /// Engineering unit.
    pub unit: &'static str,
    /// Raw value before scaling.
    pub raw_value: u64,
}

/// J1939 CAN ID components.
///
/// A 29-bit extended CAN ID broken down into J1939 fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct J1939Id {
    /// Message priority (0-7, lower is higher priority).
    pub priority: u8,
    /// Parameter Group Number.
    pub pgn: u32,
    /// Source Address (SA) of the transmitting ECU.
    pub source_address: u8,
    /// Destination Address (for PDU1 format, PS field).
    pub destination_address: u8,
}

impl J1939Id {
    /// Check if this is a broadcast message (PDU2 format).
    #[inline]
    pub fn is_broadcast(&self) -> bool {
        // PDU2 format: PF >= 240
        let pf = ((self.pgn >> 8) & 0xFF) as u8;
        pf >= 240
    }

    /// Check if this is a peer-to-peer message (PDU1 format).
    #[inline]
    pub fn is_peer_to_peer(&self) -> bool {
        !self.is_broadcast()
    }

    /// Build a 29-bit CAN ID from J1939 components.
    pub fn to_can_id(&self) -> u32 {
        let dp = (self.pgn >> 16) & 0x01;
        let pf = (self.pgn >> 8) & 0xFF;
        let ps = if pf >= 240 {
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
