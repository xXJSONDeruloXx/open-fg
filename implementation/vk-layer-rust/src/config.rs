#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    PassThrough,
    ClearTest,
    CopyTest,
    HistoryCopyTest,
}

impl Mode {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "clear" | "clear-test" => Self::ClearTest,
            "copy" | "copy-test" | "duplicate" => Self::CopyTest,
            "history" | "history-copy" | "copy-prev" | "history-copy-test" => Self::HistoryCopyTest,
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
        assert_eq!(Mode::from_env_value(Some("wat")), Mode::PassThrough);
    }

    #[test]
    fn returns_stable_mode_names() {
        assert_eq!(Mode::PassThrough.name(), "passthrough");
        assert_eq!(Mode::ClearTest.name(), "clear-test");
        assert_eq!(Mode::CopyTest.name(), "copy-test");
        assert_eq!(Mode::HistoryCopyTest.name(), "history-copy-test");
    }
}
