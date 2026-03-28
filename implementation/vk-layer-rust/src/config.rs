#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    PassThrough,
    ClearTest,
    BfiTest,
    CopyTest,
    HistoryCopyTest,
    BlendTest,
    AdaptiveBlendTest,
    SearchBlendTest,
    SearchAdaptiveBlendTest,
    ReprojectBlendTest,
    ReprojectAdaptiveBlendTest,
    OptFlowBlendTest,
    OptFlowAdaptiveBlendTest,
    OptFlowMultiBlendTest,
    OptFlowAdaptiveMultiBlendTest,
    ReprojectMultiBlendTest,
    ReprojectAdaptiveMultiBlendTest,
    MultiBlendTest,
    AdaptiveMultiBlendTest,
}

impl Mode {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "clear" | "clear-test" => Self::ClearTest,
            "bfi" | "black-frame" | "black-frame-insertion" | "bfi-test" => Self::BfiTest,
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
            "optflow-blend" | "optical-flow" | "optical-flow-blend" | "optflow-blend-test" => {
                Self::OptFlowBlendTest
            }
            "optflow-adaptive-blend"
            | "optflow-adaptive"
            | "optical-flow-adaptive"
            | "optflow-adaptive-blend-test" => Self::OptFlowAdaptiveBlendTest,
            "optflow-multi-blend"
            | "optflow-multi-fg"
            | "optflow-multi"
            | "optical-flow-multi"
            | "optflow-multi-blend-test" => Self::OptFlowMultiBlendTest,
            "optflow-adaptive-multi-blend"
            | "optflow-adaptive-multi-fg"
            | "optflow-adaptive-multi"
            | "optical-flow-adaptive-multi"
            | "optflow-adaptive-multi-blend-test" => Self::OptFlowAdaptiveMultiBlendTest,
            "reproject-multi-blend"
            | "reproject-multi-fg"
            | "reproject-multi-blend-test"
            | "multi-reproject-blend" => Self::ReprojectMultiBlendTest,
            "reproject-adaptive-multi-blend"
            | "adaptive-reproject-multi-blend"
            | "reproject-adaptive-multi-fg"
            | "reproject-adaptive-multi-blend-test" => Self::ReprojectAdaptiveMultiBlendTest,
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
        let value = crate::env_string("OMFG_LAYER_MODE");
        Self::from_env_value(value.as_deref())
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::PassThrough => "passthrough",
            Self::ClearTest => "clear-test",
            Self::BfiTest => "bfi-test",
            Self::CopyTest => "copy-test",
            Self::HistoryCopyTest => "history-copy-test",
            Self::BlendTest => "blend-test",
            Self::AdaptiveBlendTest => "adaptive-blend-test",
            Self::SearchBlendTest => "search-blend-test",
            Self::SearchAdaptiveBlendTest => "search-adaptive-blend-test",
            Self::ReprojectBlendTest => "reproject-blend-test",
            Self::ReprojectAdaptiveBlendTest => "reproject-adaptive-blend-test",
            Self::OptFlowBlendTest => "optflow-blend-test",
            Self::OptFlowAdaptiveBlendTest => "optflow-adaptive-blend-test",
            Self::OptFlowMultiBlendTest => "optflow-multi-blend-test",
            Self::OptFlowAdaptiveMultiBlendTest => "optflow-adaptive-multi-blend-test",
            Self::ReprojectMultiBlendTest => "reproject-multi-blend-test",
            Self::ReprojectAdaptiveMultiBlendTest => "reproject-adaptive-multi-blend-test",
            Self::MultiBlendTest => "multi-blend-test",
            Self::AdaptiveMultiBlendTest => "adaptive-multi-blend-test",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugView {
    Off,
    Motion,
    Confidence,
    Ambiguity,
    Disocclusion,
    HoleFill,
    Fallback,
}

impl DebugView {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "motion" | "vector" | "offset" | "reprojection-offset" => Self::Motion,
            "confidence" | "reproject-confidence" => Self::Confidence,
            "ambiguity" | "reproject-ambiguity" => Self::Ambiguity,
            "disocclusion" | "reproject-disocclusion" | "occlusion" => Self::Disocclusion,
            "hole-fill" | "holefill" | "reproject-hole-fill" => Self::HoleFill,
            "fallback" | "source" | "fallback-source" => Self::Fallback,
            _ => Self::Off,
        }
    }

    pub fn from_env() -> Self {
        let value = crate::env_string("OMFG_DEBUG_VIEW");
        Self::from_env_value(value.as_deref())
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Motion => "motion",
            Self::Confidence => "confidence",
            Self::Ambiguity => "ambiguity",
            Self::Disocclusion => "disocclusion",
            Self::HoleFill => "hole-fill",
            Self::Fallback => "fallback",
        }
    }

    pub const fn shader_code(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Motion => 1,
            Self::Confidence => 2,
            Self::Ambiguity => 3,
            Self::Disocclusion => 4,
            Self::HoleFill => 5,
            Self::Fallback => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DebugView, Mode};

    #[test]
    fn parses_mode_aliases() {
        assert_eq!(Mode::from_env_value(None), Mode::PassThrough);
        assert_eq!(Mode::from_env_value(Some("")), Mode::PassThrough);
        assert_eq!(Mode::from_env_value(Some("clear")), Mode::ClearTest);
        assert_eq!(Mode::from_env_value(Some("clear-test")), Mode::ClearTest);
        assert_eq!(Mode::from_env_value(Some("bfi")), Mode::BfiTest);
        assert_eq!(Mode::from_env_value(Some("black-frame")), Mode::BfiTest);
        assert_eq!(
            Mode::from_env_value(Some("black-frame-insertion")),
            Mode::BfiTest
        );
        assert_eq!(Mode::from_env_value(Some("bfi-test")), Mode::BfiTest);
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
            Mode::from_env_value(Some("optflow-blend")),
            Mode::OptFlowBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optical-flow")),
            Mode::OptFlowBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optical-flow-blend")),
            Mode::OptFlowBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-blend-test")),
            Mode::OptFlowBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-blend")),
            Mode::OptFlowAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive")),
            Mode::OptFlowAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optical-flow-adaptive")),
            Mode::OptFlowAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-blend-test")),
            Mode::OptFlowAdaptiveBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-multi-blend")),
            Mode::OptFlowMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-multi-fg")),
            Mode::OptFlowMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-multi")),
            Mode::OptFlowMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optical-flow-multi")),
            Mode::OptFlowMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-multi-blend-test")),
            Mode::OptFlowMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-multi-blend")),
            Mode::OptFlowAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-multi-fg")),
            Mode::OptFlowAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-multi")),
            Mode::OptFlowAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optical-flow-adaptive-multi")),
            Mode::OptFlowAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("optflow-adaptive-multi-blend-test")),
            Mode::OptFlowAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-multi-blend")),
            Mode::ReprojectMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-multi-fg")),
            Mode::ReprojectMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-multi-blend-test")),
            Mode::ReprojectMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("multi-reproject-blend")),
            Mode::ReprojectMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-adaptive-multi-blend")),
            Mode::ReprojectAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("adaptive-reproject-multi-blend")),
            Mode::ReprojectAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-adaptive-multi-fg")),
            Mode::ReprojectAdaptiveMultiBlendTest
        );
        assert_eq!(
            Mode::from_env_value(Some("reproject-adaptive-multi-blend-test")),
            Mode::ReprojectAdaptiveMultiBlendTest
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
        assert_eq!(Mode::BfiTest.name(), "bfi-test");
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
        assert_eq!(Mode::OptFlowBlendTest.name(), "optflow-blend-test");
        assert_eq!(
            Mode::OptFlowAdaptiveBlendTest.name(),
            "optflow-adaptive-blend-test"
        );
        assert_eq!(
            Mode::OptFlowMultiBlendTest.name(),
            "optflow-multi-blend-test"
        );
        assert_eq!(
            Mode::OptFlowAdaptiveMultiBlendTest.name(),
            "optflow-adaptive-multi-blend-test"
        );
        assert_eq!(
            Mode::ReprojectMultiBlendTest.name(),
            "reproject-multi-blend-test"
        );
        assert_eq!(
            Mode::ReprojectAdaptiveMultiBlendTest.name(),
            "reproject-adaptive-multi-blend-test"
        );
        assert_eq!(Mode::MultiBlendTest.name(), "multi-blend-test");
        assert_eq!(
            Mode::AdaptiveMultiBlendTest.name(),
            "adaptive-multi-blend-test"
        );
    }

    #[test]
    fn parses_debug_view_aliases() {
        assert_eq!(DebugView::from_env_value(None), DebugView::Off);
        assert_eq!(DebugView::from_env_value(Some("")), DebugView::Off);
        assert_eq!(DebugView::from_env_value(Some("motion")), DebugView::Motion);
        assert_eq!(DebugView::from_env_value(Some("vector")), DebugView::Motion);
        assert_eq!(DebugView::from_env_value(Some("offset")), DebugView::Motion);
        assert_eq!(
            DebugView::from_env_value(Some("reprojection-offset")),
            DebugView::Motion
        );
        assert_eq!(
            DebugView::from_env_value(Some("confidence")),
            DebugView::Confidence
        );
        assert_eq!(
            DebugView::from_env_value(Some("reproject-confidence")),
            DebugView::Confidence
        );
        assert_eq!(
            DebugView::from_env_value(Some("ambiguity")),
            DebugView::Ambiguity
        );
        assert_eq!(
            DebugView::from_env_value(Some("reproject-ambiguity")),
            DebugView::Ambiguity
        );
        assert_eq!(
            DebugView::from_env_value(Some("disocclusion")),
            DebugView::Disocclusion
        );
        assert_eq!(
            DebugView::from_env_value(Some("reproject-disocclusion")),
            DebugView::Disocclusion
        );
        assert_eq!(
            DebugView::from_env_value(Some("occlusion")),
            DebugView::Disocclusion
        );
        assert_eq!(
            DebugView::from_env_value(Some("hole-fill")),
            DebugView::HoleFill
        );
        assert_eq!(
            DebugView::from_env_value(Some("holefill")),
            DebugView::HoleFill
        );
        assert_eq!(
            DebugView::from_env_value(Some("reproject-hole-fill")),
            DebugView::HoleFill
        );
        assert_eq!(
            DebugView::from_env_value(Some("fallback")),
            DebugView::Fallback
        );
        assert_eq!(
            DebugView::from_env_value(Some("source")),
            DebugView::Fallback
        );
        assert_eq!(
            DebugView::from_env_value(Some("fallback-source")),
            DebugView::Fallback
        );
        assert_eq!(DebugView::from_env_value(Some("wat")), DebugView::Off);
    }

    #[test]
    fn returns_stable_debug_view_names_and_codes() {
        assert_eq!(DebugView::Off.name(), "off");
        assert_eq!(DebugView::Motion.name(), "motion");
        assert_eq!(DebugView::Confidence.name(), "confidence");
        assert_eq!(DebugView::Ambiguity.name(), "ambiguity");
        assert_eq!(DebugView::Disocclusion.name(), "disocclusion");
        assert_eq!(DebugView::HoleFill.name(), "hole-fill");
        assert_eq!(DebugView::Fallback.name(), "fallback");
        assert_eq!(DebugView::Off.shader_code(), 0);
        assert_eq!(DebugView::Motion.shader_code(), 1);
        assert_eq!(DebugView::Confidence.shader_code(), 2);
        assert_eq!(DebugView::Ambiguity.shader_code(), 3);
        assert_eq!(DebugView::Disocclusion.shader_code(), 4);
        assert_eq!(DebugView::HoleFill.shader_code(), 5);
        assert_eq!(DebugView::Fallback.shader_code(), 6);
    }
}
