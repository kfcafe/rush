/// Bash compatibility database for Rush
///
/// Provides queryable interface to the feature compatibility data.
/// Easy to extend with new features and update support status as Rush evolves.
use super::features::{rush_compat_features, RushCompatFeature, RushSupportStatus};

/// Database for querying bash feature compatibility in Rush
pub struct CompatDatabase;

impl CompatDatabase {
    /// Get all features
    pub fn all_features() -> Vec<RushCompatFeature> {
        rush_compat_features()
    }

    /// Get feature by ID
    pub fn find_feature(id: &str) -> Option<RushCompatFeature> {
        rush_compat_features().into_iter().find(|f| f.id == id)
    }

    /// Get all supported features
    pub fn supported_features() -> Vec<RushCompatFeature> {
        rush_compat_features()
            .into_iter()
            .filter(|f| f.rush_status == RushSupportStatus::Supported)
            .collect()
    }

    /// Get all planned features
    pub fn planned_features() -> Vec<RushCompatFeature> {
        rush_compat_features()
            .into_iter()
            .filter(|f| f.rush_status == RushSupportStatus::Planned)
            .collect()
    }

    /// Get all unsupported features
    pub fn unsupported_features() -> Vec<RushCompatFeature> {
        rush_compat_features()
            .into_iter()
            .filter(|f| f.rush_status == RushSupportStatus::NotSupported)
            .collect()
    }

    /// Check if a feature is supported
    pub fn is_supported(feature_id: &str) -> bool {
        Self::find_feature(feature_id)
            .map(|f| f.rush_status == RushSupportStatus::Supported)
            .unwrap_or(false)
    }

    /// Get workaround for unsupported feature
    pub fn get_workaround(feature_id: &str) -> Option<String> {
        Self::find_feature(feature_id).and_then(|f| {
            if f.rush_status == RushSupportStatus::NotSupported {
                f.workaround.map(|w| w.to_string())
            } else {
                None
            }
        })
    }

    /// Get summary statistics
    pub fn summary() -> CompatSummary {
        let features = rush_compat_features();
        let total = features.len();
        let supported = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Supported)
            .count();
        let planned = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Planned)
            .count();
        let not_supported = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::NotSupported)
            .count();

        CompatSummary {
            total,
            supported,
            planned,
            not_supported,
            support_percentage: if total > 0 {
                (supported * 100) / total
            } else {
                0
            },
        }
    }

    /// Get migration guide for unsupported features
    pub fn migration_guide() -> Vec<MigrationStep> {
        Self::unsupported_features()
            .into_iter()
            .filter_map(|f| {
                f.workaround.map(|w| MigrationStep {
                    feature_id: f.id.to_string(),
                    feature_name: f.name.to_string(),
                    bash_example: f.bash_example.to_string(),
                    workaround: w.to_string(),
                })
            })
            .collect()
    }

    /// Generate markdown documentation
    pub fn to_markdown() -> String {
        let features = Self::all_features();
        let summary = Self::summary();
        let mut output = String::new();

        output.push_str("# Rush Bash Compatibility Database\n\n");
        output.push_str(&format!(
            "**Total Features:** {} | **Supported:** {} ({})% | **Planned:** {} | **Not Supported:** {}\n\n",
            summary.total, summary.supported, summary.support_percentage, summary.planned, summary.not_supported
        ));

        output.push_str("## Features by Support Status\n\n");

        // Supported features
        output.push_str("### Supported Features\n\n");
        let supported: Vec<_> = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Supported)
            .collect();
        for feature in supported {
            output.push_str(&format!(
                "- **{}** (`{}`): {}\n",
                feature.name, feature.id, feature.description
            ));
            if let Some(rv) = feature.rush_version {
                output.push_str(&format!("  - Rush: {}\n", rv));
            }
        }
        output.push('\n');

        // Planned features
        output.push_str("### Planned Features\n\n");
        let planned: Vec<_> = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Planned)
            .collect();
        for feature in planned {
            output.push_str(&format!(
                "- **{}** (`{}`): {}\n",
                feature.name, feature.id, feature.description
            ));
            if let Some(workaround) = feature.workaround {
                output.push_str(&format!("  - Workaround: {}\n", workaround));
            }
        }
        output.push('\n');

        // Unsupported with workarounds
        output.push_str("### Not Supported (with Workarounds)\n\n");
        let not_supported: Vec<_> = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::NotSupported)
            .collect();
        for feature in not_supported {
            output.push_str(&format!(
                "- **{}** (`{}`): {}\n",
                feature.name, feature.id, feature.description
            ));
            output.push_str(&format!("  - Bash Example: `{}`\n", feature.bash_example));
            if let Some(workaround) = feature.workaround {
                output.push_str(&format!("  - Workaround: {}\n", workaround));
            }
            output.push_str(&format!("  - Notes: {}\n", feature.notes));
        }
        output.push('\n');

        output.push_str("## Statistics\n\n");
        output.push_str(&format!(
            "- Total Bash Features Catalogued: {}\n",
            summary.total
        ));
        output.push_str(&format!(
            "- Supported in Rush: {} ({})%\n",
            summary.supported, summary.support_percentage
        ));
        output.push_str(&format!("- Planned for Rush: {}\n", summary.planned));
        output.push_str(&format!(
            "- Not Supported (with workarounds): {}\n",
            summary.not_supported
        ));

        output
    }
}

/// Summary statistics
#[derive(Debug, Clone)]
pub struct CompatSummary {
    pub total: usize,
    pub supported: usize,
    pub planned: usize,
    pub not_supported: usize,
    pub support_percentage: usize,
}

/// Migration guide step
#[derive(Debug, Clone)]
pub struct MigrationStep {
    pub feature_id: String,
    pub feature_name: String,
    pub bash_example: String,
    pub workaround: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_count() {
        assert_eq!(
            CompatDatabase::all_features().len(),
            CompatDatabase::summary().total
        );
    }

    #[test]
    fn test_find_feature() {
        let feature = CompatDatabase::find_feature("echo");
        assert!(feature.is_some());
        assert_eq!(feature.unwrap().name, "Echo Builtin");
    }

    #[test]
    fn test_is_supported() {
        assert!(CompatDatabase::is_supported("echo"));
        assert!(CompatDatabase::is_supported("for-loop"));
        assert!(!CompatDatabase::is_supported("process-subst"));
    }

    #[test]
    fn test_get_workaround() {
        let workaround = CompatDatabase::get_workaround("process-subst");
        assert!(workaround.is_some());

        let workaround = CompatDatabase::get_workaround("echo");
        assert!(workaround.is_none());
    }

    #[test]
    fn test_migration_guide() {
        let guide = CompatDatabase::migration_guide();
        assert!(!guide.is_empty());
        assert!(guide.iter().all(|s| !s.workaround.is_empty()));
    }

    #[test]
    fn test_summary() {
        let summary = CompatDatabase::summary();
        assert!(summary.support_percentage <= 100);
        assert_eq!(
            summary.total,
            summary.supported + summary.planned + summary.not_supported
        );
    }

    #[test]
    fn test_markdown() {
        let md = CompatDatabase::to_markdown();
        assert!(md.contains("# Rush Bash Compatibility Database"));
        assert!(md.contains("Supported Features"));
        assert!(md.contains("Planned Features"));
        assert!(md.contains("Not Supported"));
    }
}
