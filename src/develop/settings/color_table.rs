use serde::{Deserialize, Serialize};

use super::{SettingsError, validate_range};

/// Smallest and largest edge length the table may have. Below nine entries an
/// edge cannot hold a photographic rendering without visible banding between
/// samples; above sixty-five the table costs more to carry in a preset than
/// the extra precision is worth.
pub const TABLE_MIN_SIZE: u32 = 9;
pub const TABLE_MAX_SIZE: u32 = 65;

/// A colour rendering held as a lookup over the whole colour space.
///
/// Unlike the mixer's hue bands, this can treat two colours of the same hue
/// differently according to how light or saturated they are, which is what a
/// photographic colour rendering generally does.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ColorTableSettings {
    /// Edge length of the cube. Zero means no table is present.
    pub size: u32,
    /// Red-major triples, `size³ × 3` of them, in display-encoded coordinates.
    pub entries: Vec<f32>,
    /// How much of the table's move to apply, as a percentage.
    pub strength: f32,
}

impl ColorTableSettings {
    pub fn is_neutral(&self) -> bool {
        self.size == 0 || self.entries.is_empty() || self.strength == 0.0
    }

    /// Nothing set at all — the form every preset written before this stage
    /// existed has, and the only form that is left out when serialising.
    pub fn is_default(&self) -> bool {
        self.size == 0 && self.entries.is_empty() && self.strength == 0.0
    }

    /// Whether the declared size and the entries agree, and every entry is a
    /// finite number. A table that fails this is a broken preset, not a
    /// creative choice, so the stage reports it rather than rendering it.
    pub fn is_well_formed(&self) -> bool {
        self.size >= TABLE_MIN_SIZE
            && self.size <= TABLE_MAX_SIZE
            && self.entries.len() == (self.size as usize).pow(3) * 3
            && self.entries.iter().all(|entry| entry.is_finite())
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        validate_range("color_table.strength", self.strength, 0.0, 100.0)?;
        if self.size == 0 {
            if self.entries.is_empty() {
                return Ok(());
            }
            return Err(SettingsError::new(
                "color_table.size",
                "a table with no size must have no entries",
            ));
        }
        validate_range(
            "color_table.size",
            self.size as f32,
            TABLE_MIN_SIZE as f32,
            TABLE_MAX_SIZE as f32,
        )?;
        if !self.is_well_formed() {
            return Err(SettingsError::new(
                "color_table.entries",
                format!(
                    "a table of size {} needs {} finite entries, found {}",
                    self.size,
                    (self.size as usize).pow(3) * 3,
                    self.entries.len()
                ),
            ));
        }
        Ok(())
    }

    pub fn canonicalize(&mut self) {
        // A table with no strength carries nothing worth keeping. A strength
        // with no table is kept as it is: it says how much of a table to
        // apply once one is present, and dropping it would make the control
        // impossible to set before the table is loaded.
        if self.strength == 0.0 || self.size == 0 {
            self.size = 0;
            self.entries.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_without_entries_is_neutral_and_valid() {
        let settings = ColorTableSettings::default();
        assert!(settings.is_neutral());
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn size_and_entry_count_must_agree() {
        let mut settings = ColorTableSettings {
            size: 9,
            entries: vec![0.0; 9 * 9 * 9 * 3],
            strength: 100.0,
        };
        assert!(settings.validate().is_ok());
        settings.entries.pop();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn canonicalizing_a_disabled_table_drops_its_entries() {
        let mut settings = ColorTableSettings {
            size: 9,
            entries: vec![0.0; 9 * 9 * 9 * 3],
            strength: 0.0,
        };
        settings.canonicalize();
        assert_eq!(settings, ColorTableSettings::default());
    }
}
