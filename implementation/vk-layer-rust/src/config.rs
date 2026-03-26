#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    PassThrough,
    ClearTest,
    CopyTest,
    HistoryCopyTest,
    BlendTest,
    AdaptiveBlendTest,
    SearchBlendTest,
    SearchAdaptiveBlendTest,
    ReprojectBlendTest,
    ReprojectAdaptiveBlendTest,
    MultiBlendTest,
    AdaptiveMultiBlendTest,
}

impl Mode {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "clear" | "clear-test" => Self::ClearTest,
            "copy" | "copy-test" | "duplicate" => Self::CopyTest,
            "history" | "history-copy" | "copy-prev" | "history-copy-test" => Self::HistoryCopyTest,
            "blend" | "blend-test" | "history-blend" | "blend-prev-current" => Self::BlendTest,
            "adaptive-blend" | "adaptive" | "adaptive-blend-test" | "blend-adaptive" => {
                Self::AdaptiveBlendTest
            }
            "search-blend" | "motion-search" | "motion-search-blend" | "search-blend-test" => {
                Self::SearchBlendTest
            }
            "search-adaptive-blend"
            | "adaptive-search-blend"
            | "motion-search-adaptive"
            | "search-adaptive-blend-test" => Self::SearchAdaptiveBlendTest,
            "reproject-blend"
            | "vector-reproject-blend"
            | "motion-reproject"
            | "reproject-blend-test" => Self::ReprojectBlendTest,
            "reproject-adaptive-blend"
            | "adaptive-reproject-blend"
            | "vector-reproject-adaptive"
            | "reproject-adaptive-blend-test" => Self::ReprojectAdaptiveBlendTest,
            "multi-blend" | "multi-fg" | "multi-fg-test" | "multi-blend-test" => {
                Self::MultiBlendTest
            }
            "adaptive-multi-blend"
            | "adaptive-multi-fg"
            | "adaptive-multi-blend-test"
            | "multi-blend-adaptive" => Self::AdaptiveMultiBlendTest,
            _ => Self::PassThrough,
        }
    }

    pub fn from_env() -> Self {
        Self::from_env_value(std::env::var("PPFG_LAYER_MODE").ok().as_deref())
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::PassThrough => "passthrough",
            Self::ClearTest => "clear-test",
            Self::CopyTest => "copy-test",
            Self::HistoryCopyTest => "history-copy-test",
            Self::BlendTest => "blend-test",
            Self::AdaptiveBlendTest => "adaptive-blend-test",
            Self::SearchBlendTest => "search-blend-test",
            Self::SearchAdaptiveBlendTest => "search-adaptive-blend-test",
            Self::ReprojectBlendTest => "reproject-blend-test",
            Self::ReprojectAdaptiveBlendTest => "reproject-adaptive-blend-test",
            Self::MultiBlendTest => "multi-blend-test",
            Self::AdaptiveMultiBlendTest => "adaptive-multi-blend-test",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;

    #[test]
    fn parses_mode_aliases() {
        assert_eq!(Mode::from_env_value(None), Mode::PassThrough);
        assert_eq!(Mode::from_env_value(Some("")), Mode::PassThrough);
        assert_eq!(Mode::from_env_value(Some("clear")), Mode::ClearTest);
        assert_eq!(Mode::from_env_value(Some("clear-test")), Mode::ClearTest);
        assert_eq!(Mode::from_env_value(Some("copy")), Mode::CopyTest);
        assert_eq!(Mode::from_env_value(Some("copy-test")), Mode::CopyTest);
        assert_eq!(Mode::from_env_value(Some("duplicate")), Mode::CopyTest);
        assert_eq!(Mode::from_env_value(Some("history")), Mode::HistoryCopyTest);
        assert_eq!(
            Mode::from_env_value(Some("history-copy")),
            Mode::HistoryCopyTest
        );
        assert_eq!(
            Mode::from_env_value(Some("copy-prev")),
            Mode::HistoryCopyTest
        );
        assert_eq!(
            Mode::from_env_value(Some("history-copy-test")),
            Mode::HistoryCopyTest
        );
        assert_eq!(Mode::from_env_value(Some("blend")), Mode::BlendTest);
        assert_eq!(Mode::from_env_value(Some("blend-test")), Mode::BlendTest);
        assert_eq!(Mode::from_env_value(Some("history-blend")), Mode::BlendTest);
        assert_eq!(
            Mode::from_env_value(Some("blend-prev-current")),
            Mode::BlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-blend")),
            Mode::AdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive")),
            Mode::AdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-blend-test")),
            Mode::AdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("blend-adaptive")),
            Mode::AdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("search-blend")),
            Mode::SearchBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("motion-search")),
            Mode::SearchBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("motion-search-blend")),
            Mode::SearchBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("search-blend-test")),
            Mode::SearchBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("search-adaptive-blend")),
            Mode::SearchAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-search-blend")),
            Mode::SearchAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("motion-search-adaptive")),
            Mode::SearchAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("search-adaptive-blend-test")),
            Mode::SearchAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-blend")),
            Mode::ReprojectBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("vector-reproject-blend")),
            Mode::ReprojectBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("motion-reproject")),
            Mode::ReprojectBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-blend-test")),
            Mode::ReprojectBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-adaptive-blend")),
            Mode::ReprojectAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-reproject-blend")),
            Mode::ReprojectAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("vector-reproject-adaptive")),
            Mode::ReprojectAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-adaptive-blend-test")),
            Mode::ReprojectAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("multi-blend")),
            Mode::MultiBlendTest
        );
        assert_eq!(Mode::from_env_value(Some("multi-fg")), Mode::MultiBlendTest);
        assert_eq!(
            Mode::from_env_value(Some("multi-fg-test")),
            Mode::MultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("multi-blend-test")),
            Mode::MultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-multi-blend")),
            Mode::AdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-multi-fg")),
            Mode::AdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-multi-blend-test")),
            Mode::AdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("multi-blend-adaptive")),
            Mode::AdaptiveMultiBlendTest
        );
        assert_eq!(Mode::from_env_value(Some("wat")), Mode::PassThrough);
    }

    #[test]
    fn returns_stable_mode_names() {
        assert_eq!(Mode::PassThrough.name(), "passthrough");
        assert_eq!(Mode::ClearTest.name(), "clear-test");
        assert_eq!(Mode::CopyTest.name(), "copy-test");
        assert_eq!(Mode::HistoryCopyTest.name(), "history-copy-test");
        assert_eq!(Mode::BlendTest.name(), "blend-test");
        assert_eq!(Mode::AdaptiveBlendTest.name(), "adaptive-blend-test");
        assert_eq!(Mode::SearchBlendTest.name(), "search-blend-test");
        assert_eq!(
            Mode::SearchAdaptiveBlendTest.name(),
            "search-adaptive-blend-test"
        );
        assert_eq!(Mode::ReprojectBlendTest.name(), "reproject-blend-test");
        assert_eq!(
            Mode::ReprojectAdaptiveBlendTest.name(),
            "reproject-adaptive-blend-test"
        );
        assert_eq!(Mode::MultiBlendTest.name(), "multi-blend-test");
        assert_eq!(
            Mode::AdaptiveMultiBlendTest.name(),
            "adaptive-multi-blend-test"
        );
    }
}
