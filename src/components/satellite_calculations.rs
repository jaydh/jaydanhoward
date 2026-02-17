#![allow(clippy::all)]
//! Satellite orbital calculations using SGP4

use sgp4::{Constants, Elements};

#[derive(Clone, Debug)]
pub struct SatellitePosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone)]
pub struct Satellite {
    pub name: String,
    constants: Constants,
    epoch_j2000_years: f64,
}

impl Satellite {
    /// Create a new satellite from TLE data
    pub fn from_tle(name: String, line1: &str, line2: &str) -> Result<Self, String> {
        // Parse TLE
        let elements = Elements::from_tle(
            Some(name.clone()),
            line1.as_bytes(),
            line2.as_bytes(),
        ).map_err(|e| format!("Failed to parse TLE: {:?}", e))?;

        // Create propagation constants
        let constants = Constants::from_elements(&elements)
            .map_err(|e| format!("Failed to create constants: {:?}", e))?;

        // Get epoch time (Julian years since J2000)
        let epoch_j2000_years = elements.epoch();

        Ok(Self {
            name,
            constants,
            epoch_j2000_years,
        })
    }

    /// Calculate satellite position at a given time (minutes since epoch)
    pub fn propagate(&self, minutes_since_epoch: f64) -> Result<SatellitePosition, String> {
        let prediction = self.constants.propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))
            .map_err(|e| format!("Failed to propagate: {:?}", e))?;

        // Position is in kilometers in ECI (Earth-Centered Inertial) coordinates
        // Scale down to Earth radius units (Earth radius ~6371 km)
        const EARTH_RADIUS_KM: f64 = 6371.0;
        let scale = 1.0 / EARTH_RADIUS_KM;

        Ok(SatellitePosition {
            x: (prediction.position[0] * scale) as f32,
            y: (prediction.position[1] * scale) as f32,
            z: (prediction.position[2] * scale) as f32,
        })
    }

    /// Calculate current position based on system time
    pub fn current_position(&self) -> Result<SatellitePosition, String> {
        let now_ms = js_sys::Date::now();
        // J2000 epoch: 2000-01-01 12:00:00 UTC = 946728000000 ms Unix
        let j2000_unix_ms = 946_728_000_000.0;
        let minutes_since_j2000 = (now_ms - j2000_unix_ms) / 60_000.0;
        let epoch_minutes_from_j2000 = self.epoch_j2000_years * 365.25 * 24.0 * 60.0;
        let minutes_since_epoch = minutes_since_j2000 - epoch_minutes_from_j2000;
        self.propagate(minutes_since_epoch)
    }
}

/// Parse multiple TLE entries and create satellites
pub fn parse_satellites(tle_data: &[(String, String, String)]) -> Vec<Satellite> {
    tle_data
        .iter()
        .filter_map(|(name, line1, line2)| {
            match Satellite::from_tle(name.clone(), line1, line2) {
                Ok(sat) => Some(sat),
                Err(e) => {
                    #[cfg(not(feature = "ssr"))]
                    web_sys::console::warn_1(&format!("Failed to parse satellite {}: {}", name, e).into());
                    None
                }
            }
        })
        .collect()
}

/// Calculate positions for all satellites at current time
pub fn calculate_positions(satellites: &[Satellite]) -> Vec<(String, SatellitePosition)> {
    satellites
        .iter()
        .filter_map(|sat| {
            match sat.current_position() {
                Ok(pos) => Some((sat.name.clone(), pos)),
                Err(_) => None,
            }
        })
        .collect()
}
