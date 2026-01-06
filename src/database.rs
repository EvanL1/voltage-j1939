//! J1939 SPN Database
//!
//! Complete database of common engine/generator SPNs based on SAE J1939 standard.
//! Point ID = SPN (globally unique per SAE J1939 standard).
//!
//! This database covers the most commonly used PGNs for diesel generators and
//! industrial engines. Data is automatically decoded when matching PGNs are received.

use std::sync::OnceLock;

use crate::types::{SpnDataType, SpnDef};

// ============================================================================
// Lazy-initialized lookup tables for O(1) access
// ============================================================================

/// Cached PGN -> SPNs mapping (lazy initialized)
static PGN_LOOKUP: OnceLock<PgnLookup> = OnceLock::new();

/// Cached SPN -> SpnDef mapping (lazy initialized)
static SPN_LOOKUP: OnceLock<SpnLookup> = OnceLock::new();

/// Compact PGN lookup structure using Box<[T]> for minimal memory footprint.
/// Box<[T]> saves 8 bytes per field vs Vec (no capacity field needed).
struct PgnLookup {
    /// Sorted list of (pgn, start_idx, count) for binary search
    index: Box<[(u32, u16, u16)]>,
    /// Flattened array of SpnDef references, grouped by PGN
    spns: Box<[&'static SpnDef]>,
}

/// Compact SPN lookup using sorted array + binary search (faster than HashMap for small N).
/// Box<[T]> saves 8 bytes vs Vec (no capacity field needed).
struct SpnLookup {
    /// Sorted by SPN number for binary search
    entries: Box<[(u32, &'static SpnDef)]>,
}

impl PgnLookup {
    fn build() -> Self {
        // O(n log n): Sort SPNs by PGN first
        let mut sorted_spns: Vec<&'static SpnDef> = SPN_DEFINITIONS.iter().collect();
        sorted_spns.sort_unstable_by_key(|s| s.pgn);

        // Count unique PGNs for pre-allocation (O(n) but worth it for no realloc)
        let pgn_count = sorted_spns
            .windows(2)
            .filter(|w| w[0].pgn != w[1].pgn)
            .count()
            + 1;

        // O(n): Single pass to build index
        let mut index: Vec<(u32, u16, u16)> = Vec::with_capacity(pgn_count);
        let mut current_pgn = u32::MAX;
        let mut start_idx = 0u16;

        for (i, spn) in sorted_spns.iter().enumerate() {
            if spn.pgn != current_pgn {
                // Finalize previous PGN group
                if current_pgn != u32::MAX {
                    let count = (i as u16) - start_idx;
                    index.push((current_pgn, start_idx, count));
                }
                current_pgn = spn.pgn;
                start_idx = i as u16;
            }
        }
        // Finalize last PGN group
        if current_pgn != u32::MAX {
            let count = (sorted_spns.len() as u16) - start_idx;
            index.push((current_pgn, start_idx, count));
        }

        Self {
            index: index.into_boxed_slice(),
            spns: sorted_spns.into_boxed_slice(),
        }
    }

    /// Hot path for frame decoding - always inlined.
    #[inline(always)]
    fn get(&self, pgn: u32) -> Option<&[&'static SpnDef]> {
        match self.index.binary_search_by_key(&pgn, |(p, _, _)| *p) {
            Ok(idx) => {
                let (_, start, count) = self.index[idx];
                Some(&self.spns[start as usize..(start + count) as usize])
            }
            Err(_) => None, // PGN not found - cold path
        }
    }

    #[inline]
    fn pgn_count(&self) -> usize {
        self.index.len()
    }

    #[inline]
    fn iter_pgns(&self) -> impl Iterator<Item = u32> + '_ {
        self.index.iter().map(|(pgn, _, _)| *pgn)
    }
}

impl SpnLookup {
    fn build() -> Self {
        // Pre-allocate with exact capacity
        let mut entries: Vec<(u32, &'static SpnDef)> =
            Vec::with_capacity(SPN_DEFINITIONS.len());
        entries.extend(SPN_DEFINITIONS.iter().map(|s| (s.spn, s)));
        entries.sort_unstable_by_key(|(spn, _)| *spn);
        Self {
            entries: entries.into_boxed_slice(),
        }
    }

    /// SPN lookup - inlined for decode_spn_by_number hot path.
    #[inline(always)]
    fn get(&self, spn: u32) -> Option<&'static SpnDef> {
        match self.entries.binary_search_by_key(&spn, |(s, _)| *s) {
            Ok(idx) => Some(self.entries[idx].1),
            Err(_) => None, // SPN not found - cold path
        }
    }
}

/// Get PGN lookup table - initialized once, then O(1) access.
/// Hot path: always inlined to avoid function call overhead.
#[inline(always)]
fn pgn_lookup() -> &'static PgnLookup {
    PGN_LOOKUP.get_or_init(PgnLookup::build)
}

/// Get SPN lookup table - initialized once, then O(1) access.
/// Hot path: always inlined to avoid function call overhead.
#[inline(always)]
fn spn_lookup() -> &'static SpnLookup {
    SPN_LOOKUP.get_or_init(SpnLookup::build)
}

// ============================================================================
// SPN Database - Complete definitions for common engine PGNs
// ============================================================================

/// All SPN definitions in the database.
pub static SPN_DEFINITIONS: &[SpnDef] = &[
    // ========================================================================
    // EEC1 - Electronic Engine Controller 1 (PGN 61444 / 0xF004)
    // Broadcast rate: 10-100ms (engine dependent)
    // ========================================================================
    SpnDef {
        spn: 899,
        name: "engine_torque_mode",
        pgn: 61444,
        start_byte: 0,
        start_bit: 0,
        bit_length: 4,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 4154,
        name: "actual_engine_retarder_percent",
        pgn: 61444,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 512,
        name: "drivers_demand_engine_percent",
        pgn: 61444,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 513,
        name: "actual_engine_percent_torque",
        pgn: 61444,
        start_byte: 2,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 190,
        name: "engine_speed",
        pgn: 61444,
        start_byte: 3,
        start_bit: 0,
        bit_length: 16,
        scale: 0.125,
        offset: 0.0,
        unit: "RPM",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 1483,
        name: "eec1_source_address",
        pgn: 61444,
        start_byte: 5,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 1675,
        name: "engine_starter_mode",
        pgn: 61444,
        start_byte: 6,
        start_bit: 0,
        bit_length: 4,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 2432,
        name: "engine_demand_percent_torque",
        pgn: 61444,
        start_byte: 7,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // EEC2 - Electronic Engine Controller 2 (PGN 61443 / 0xF003)
    // Broadcast rate: 50ms
    // ========================================================================
    SpnDef {
        spn: 558,
        name: "accelerator_pedal_1_low_switch",
        pgn: 61443,
        start_byte: 0,
        start_bit: 0,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 559,
        name: "accelerator_pedal_kickdown",
        pgn: 61443,
        start_byte: 0,
        start_bit: 2,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 1437,
        name: "road_speed_limit_status",
        pgn: 61443,
        start_byte: 0,
        start_bit: 4,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 2970,
        name: "accelerator_pedal_2_low_switch",
        pgn: 61443,
        start_byte: 0,
        start_bit: 6,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 91,
        name: "accelerator_pedal_position_1",
        pgn: 61443,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 92,
        name: "percent_load_current_speed",
        pgn: 61443,
        start_byte: 2,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 974,
        name: "remote_accelerator_position",
        pgn: 61443,
        start_byte: 3,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 29,
        name: "accelerator_pedal_position_2",
        pgn: 61443,
        start_byte: 4,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 2979,
        name: "vehicle_acceleration_rate_limit",
        pgn: 61443,
        start_byte: 5,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 5021,
        name: "momentary_engine_max_power_enable",
        pgn: 61443,
        start_byte: 6,
        start_bit: 0,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // EEC3 - Electronic Engine Controller 3 (PGN 65247 / 0xFEDF)
    // Broadcast rate: 250ms
    // ========================================================================
    SpnDef {
        spn: 514,
        name: "nominal_friction_percent_torque",
        pgn: 65247,
        start_byte: 0,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 515,
        name: "engine_desired_operating_speed",
        pgn: 65247,
        start_byte: 1,
        start_bit: 0,
        bit_length: 16,
        scale: 0.125,
        offset: 0.0,
        unit: "RPM",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 519,
        name: "engine_operating_speed_asymmetry_adjust",
        pgn: 65247,
        start_byte: 3,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 2978,
        name: "estimated_engine_parasitic_losses",
        pgn: 65247,
        start_byte: 4,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -125.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 6595,
        name: "aftertreatment_1_exhaust_gas_mass_flow",
        pgn: 65247,
        start_byte: 5,
        start_bit: 0,
        bit_length: 16,
        scale: 0.2,
        offset: 0.0,
        unit: "kg/h",
        data_type: SpnDataType::Uint16,
    },
    // ========================================================================
    // ET1 - Engine Temperature 1 (PGN 65262 / 0xFEEE)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 110,
        name: "engine_coolant_temperature",
        pgn: 65262,
        start_byte: 0,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -40.0,
        unit: "C",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 174,
        name: "fuel_temperature",
        pgn: 65262,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -40.0,
        unit: "C",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 175,
        name: "engine_oil_temperature_1",
        pgn: 65262,
        start_byte: 2,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 176,
        name: "turbo_oil_temperature",
        pgn: 65262,
        start_byte: 4,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 52,
        name: "engine_intercooler_temperature",
        pgn: 65262,
        start_byte: 6,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -40.0,
        unit: "C",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 1134,
        name: "engine_intercooler_thermostat_opening",
        pgn: 65262,
        start_byte: 7,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // EFL/P1 - Engine Fluid Level/Pressure 1 (PGN 65263 / 0xFEEF)
    // Broadcast rate: 500ms
    // ========================================================================
    SpnDef {
        spn: 94,
        name: "fuel_delivery_pressure",
        pgn: 65263,
        start_byte: 0,
        start_bit: 0,
        bit_length: 8,
        scale: 4.0,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 22,
        name: "extended_crankcase_blowby_pressure",
        pgn: 65263,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 0.05,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 98,
        name: "engine_oil_level",
        pgn: 65263,
        start_byte: 2,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 100,
        name: "engine_oil_pressure",
        pgn: 65263,
        start_byte: 3,
        start_bit: 0,
        bit_length: 8,
        scale: 4.0,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 101,
        name: "crankcase_pressure",
        pgn: 65263,
        start_byte: 4,
        start_bit: 0,
        bit_length: 16,
        scale: 0.0078125,
        offset: -250.0,
        unit: "kPa",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 109,
        name: "coolant_pressure",
        pgn: 65263,
        start_byte: 6,
        start_bit: 0,
        bit_length: 8,
        scale: 2.0,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 111,
        name: "coolant_level",
        pgn: 65263,
        start_byte: 7,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // IC1 - Inlet/Exhaust Conditions 1 (PGN 65270 / 0xFEF6)
    // Broadcast rate: 500ms
    // ========================================================================
    SpnDef {
        spn: 81,
        name: "particulate_trap_inlet_pressure",
        pgn: 65270,
        start_byte: 0,
        start_bit: 0,
        bit_length: 8,
        scale: 0.5,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 102,
        name: "boost_pressure",
        pgn: 65270,
        start_byte: 1,
        start_bit: 0,
        bit_length: 8,
        scale: 2.0,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 105,
        name: "intake_manifold_temperature",
        pgn: 65270,
        start_byte: 2,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -40.0,
        unit: "C",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 106,
        name: "air_inlet_pressure",
        pgn: 65270,
        start_byte: 3,
        start_bit: 0,
        bit_length: 8,
        scale: 2.0,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 107,
        name: "air_filter_differential_pressure",
        pgn: 65270,
        start_byte: 4,
        start_bit: 0,
        bit_length: 8,
        scale: 0.05,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 173,
        name: "exhaust_gas_temperature",
        pgn: 65270,
        start_byte: 5,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 112,
        name: "coolant_filter_differential_pressure",
        pgn: 65270,
        start_byte: 7,
        start_bit: 0,
        bit_length: 8,
        scale: 0.5,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // VEP1 - Vehicle Electrical Power 1 (PGN 65271 / 0xFEF7)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 114,
        name: "net_battery_current",
        pgn: 65271,
        start_byte: 0,
        start_bit: 0,
        bit_length: 16,
        scale: 1.0,
        offset: -125.0,
        unit: "A",
        data_type: SpnDataType::Int16,
    },
    SpnDef {
        spn: 115,
        name: "alternator_current",
        pgn: 65271,
        start_byte: 2,
        start_bit: 0,
        bit_length: 16,
        scale: 1.0,
        offset: 0.0,
        unit: "A",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 168,
        name: "battery_potential",
        pgn: 65271,
        start_byte: 4,
        start_bit: 0,
        bit_length: 16,
        scale: 0.05,
        offset: 0.0,
        unit: "V",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 158,
        name: "keyswitch_battery_potential",
        pgn: 65271,
        start_byte: 6,
        start_bit: 0,
        bit_length: 16,
        scale: 0.05,
        offset: 0.0,
        unit: "V",
        data_type: SpnDataType::Uint16,
    },
    // ========================================================================
    // AMB - Ambient Conditions (PGN 65269 / 0xFEF5)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 108,
        name: "barometric_pressure",
        pgn: 65269,
        start_byte: 0,
        start_bit: 0,
        bit_length: 8,
        scale: 0.5,
        offset: 0.0,
        unit: "kPa",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 170,
        name: "cab_interior_temperature",
        pgn: 65269,
        start_byte: 1,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 171,
        name: "ambient_air_temperature",
        pgn: 65269,
        start_byte: 3,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 172,
        name: "air_inlet_temperature",
        pgn: 65269,
        start_byte: 5,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: -40.0,
        unit: "C",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 79,
        name: "road_surface_temperature",
        pgn: 65269,
        start_byte: 6,
        start_bit: 0,
        bit_length: 16,
        scale: 0.03125,
        offset: -273.0,
        unit: "C",
        data_type: SpnDataType::Uint16,
    },
    // ========================================================================
    // LFE - Liquid Fuel Economy (PGN 65266 / 0xFEF2)
    // Broadcast rate: 100ms
    // ========================================================================
    SpnDef {
        spn: 183,
        name: "fuel_rate",
        pgn: 65266,
        start_byte: 0,
        start_bit: 0,
        bit_length: 16,
        scale: 0.05,
        offset: 0.0,
        unit: "L/h",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 184,
        name: "instantaneous_fuel_economy",
        pgn: 65266,
        start_byte: 2,
        start_bit: 0,
        bit_length: 16,
        scale: 0.001953125,
        offset: 0.0,
        unit: "km/L",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 185,
        name: "average_fuel_economy",
        pgn: 65266,
        start_byte: 4,
        start_bit: 0,
        bit_length: 16,
        scale: 0.001953125,
        offset: 0.0,
        unit: "km/L",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 51,
        name: "throttle_position",
        pgn: 65266,
        start_byte: 6,
        start_bit: 0,
        bit_length: 8,
        scale: 0.4,
        offset: 0.0,
        unit: "%",
        data_type: SpnDataType::Uint8,
    },
    // ========================================================================
    // HOURS - Engine Hours, Revolutions (PGN 65253 / 0xFEE5)
    // Broadcast rate: On request or 1000ms
    // ========================================================================
    SpnDef {
        spn: 247,
        name: "engine_total_hours_of_operation",
        pgn: 65253,
        start_byte: 0,
        start_bit: 0,
        bit_length: 32,
        scale: 0.05,
        offset: 0.0,
        unit: "h",
        data_type: SpnDataType::Uint32,
    },
    SpnDef {
        spn: 249,
        name: "engine_total_revolutions",
        pgn: 65253,
        start_byte: 4,
        start_bit: 0,
        bit_length: 32,
        scale: 1000.0,
        offset: 0.0,
        unit: "r",
        data_type: SpnDataType::Uint32,
    },
    // ========================================================================
    // FC - Fuel Consumption (PGN 65257 / 0xFEE9)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 182,
        name: "engine_trip_fuel",
        pgn: 65257,
        start_byte: 0,
        start_bit: 0,
        bit_length: 32,
        scale: 0.5,
        offset: 0.0,
        unit: "L",
        data_type: SpnDataType::Uint32,
    },
    SpnDef {
        spn: 250,
        name: "engine_total_fuel_used",
        pgn: 65257,
        start_byte: 4,
        start_bit: 0,
        bit_length: 32,
        scale: 0.5,
        offset: 0.0,
        unit: "L",
        data_type: SpnDataType::Uint32,
    },
    // ========================================================================
    // VH - Vehicle Hours (PGN 65217 / 0xFEC1)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 246,
        name: "engine_total_idle_hours",
        pgn: 65217,
        start_byte: 0,
        start_bit: 0,
        bit_length: 32,
        scale: 0.05,
        offset: 0.0,
        unit: "h",
        data_type: SpnDataType::Uint32,
    },
    SpnDef {
        spn: 248,
        name: "engine_total_pto_hours",
        pgn: 65217,
        start_byte: 4,
        start_bit: 0,
        bit_length: 32,
        scale: 0.05,
        offset: 0.0,
        unit: "h",
        data_type: SpnDataType::Uint32,
    },
    // ========================================================================
    // DD - Distance (PGN 65248 / 0xFEE0)
    // Broadcast rate: 1000ms
    // ========================================================================
    SpnDef {
        spn: 244,
        name: "trip_distance",
        pgn: 65248,
        start_byte: 0,
        start_bit: 0,
        bit_length: 32,
        scale: 0.125,
        offset: 0.0,
        unit: "km",
        data_type: SpnDataType::Uint32,
    },
    SpnDef {
        spn: 245,
        name: "total_vehicle_distance",
        pgn: 65248,
        start_byte: 4,
        start_bit: 0,
        bit_length: 32,
        scale: 0.125,
        offset: 0.0,
        unit: "km",
        data_type: SpnDataType::Uint32,
    },
    // ========================================================================
    // CCVS - Cruise Control/Vehicle Speed (PGN 65265 / 0xFEF1)
    // Broadcast rate: 100ms
    // ========================================================================
    SpnDef {
        spn: 69,
        name: "two_speed_axle_switch",
        pgn: 65265,
        start_byte: 0,
        start_bit: 0,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 70,
        name: "parking_brake_switch",
        pgn: 65265,
        start_byte: 0,
        start_bit: 2,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 84,
        name: "wheel_based_vehicle_speed",
        pgn: 65265,
        start_byte: 1,
        start_bit: 0,
        bit_length: 16,
        scale: 0.00390625,
        offset: 0.0,
        unit: "km/h",
        data_type: SpnDataType::Uint16,
    },
    SpnDef {
        spn: 595,
        name: "cruise_control_active",
        pgn: 65265,
        start_byte: 3,
        start_bit: 0,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 596,
        name: "cruise_control_enable_switch",
        pgn: 65265,
        start_byte: 3,
        start_bit: 2,
        bit_length: 2,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 86,
        name: "cruise_control_set_speed",
        pgn: 65265,
        start_byte: 5,
        start_bit: 0,
        bit_length: 8,
        scale: 1.0,
        offset: 0.0,
        unit: "km/h",
        data_type: SpnDataType::Uint8,
    },
    SpnDef {
        spn: 976,
        name: "pto_state",
        pgn: 65265,
        start_byte: 6,
        start_bit: 0,
        bit_length: 5,
        scale: 1.0,
        offset: 0.0,
        unit: "",
        data_type: SpnDataType::Uint8,
    },
];

// ============================================================================
// Database lookup functions - O(log n) via binary search
// ============================================================================

/// Get all SPNs for a given PGN.
///
/// Returns a slice of SPN definitions. O(log n) lookup via binary search.
/// Hot path: always inlined for frame decoding.
///
/// # Example
///
/// ```
/// use voltage_j1939::database::get_spns_for_pgn;
///
/// // EEC1 (PGN 61444) contains engine speed, torque, etc.
/// if let Some(spns) = get_spns_for_pgn(61444) {
///     for spn in spns {
///         println!("SPN {}: {}", spn.spn, spn.name);
///     }
/// }
/// ```
#[inline(always)]
pub fn get_spns_for_pgn(pgn: u32) -> Option<&'static [&'static SpnDef]> {
    pgn_lookup().get(pgn)
}

/// Get a specific SPN definition by SPN number.
///
/// O(log n) lookup via binary search.
/// Hot path: always inlined for decode_spn_by_number.
///
/// # Example
///
/// ```
/// use voltage_j1939::database::get_spn_def;
///
/// // SPN 190 = Engine Speed
/// if let Some(spn) = get_spn_def(190) {
///     println!("Name: {}, Scale: {}", spn.name, spn.scale);
/// }
/// ```
#[inline(always)]
pub fn get_spn_def(spn: u32) -> Option<&'static SpnDef> {
    spn_lookup().get(spn)
}

/// Get statistics about the database.
///
/// Returns (number of unique PGNs, total number of SPNs).
/// O(1) after first call (cached).
#[inline]
pub fn database_stats() -> (usize, usize) {
    (pgn_lookup().pgn_count(), SPN_DEFINITIONS.len())
}

/// List all supported PGNs (already sorted).
#[inline]
pub fn list_supported_pgns() -> impl Iterator<Item = u32> {
    pgn_lookup().iter_pgns()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_stats() {
        let (pgn_count, spn_count) = database_stats();
        assert!(pgn_count >= 10, "Should have at least 10 PGNs");
        assert!(spn_count >= 50, "Should have at least 50 SPNs");
    }

    #[test]
    fn test_get_spns_for_pgn() {
        // EEC1 should have multiple SPNs
        let spns = get_spns_for_pgn(61444);
        assert!(spns.is_some());
        let spns = spns.unwrap();
        assert!(spns.len() >= 5, "EEC1 should have at least 5 SPNs");

        // Check engine speed is present
        assert!(
            spns.iter().any(|s| s.spn == 190),
            "Should have SPN 190 (engine_speed)"
        );
    }

    #[test]
    fn test_get_spn_def() {
        // Engine speed
        let spn = get_spn_def(190);
        assert!(spn.is_some());
        let spn = spn.unwrap();
        assert_eq!(spn.name, "engine_speed");
        assert_eq!(spn.pgn, 61444);
        assert_eq!(spn.scale, 0.125);

        // Coolant temperature
        let spn = get_spn_def(110);
        assert!(spn.is_some());
        let spn = spn.unwrap();
        assert_eq!(spn.name, "engine_coolant_temperature");
        assert_eq!(spn.offset, -40.0);
    }

    #[test]
    fn test_list_supported_pgns() {
        let pgns: Vec<_> = list_supported_pgns().collect();
        assert!(!pgns.is_empty());
        assert!(pgns.contains(&61444)); // EEC1
        assert!(pgns.contains(&65262)); // ET1
    }
}
