use ash::vk;

use crate::config::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapchainMutation {
    pub modified_min_image_count: u32,
    pub modified_usage: vk::ImageUsageFlags,
}

pub fn mutate_swapchain(
    mode: Mode,
    original_min_image_count: u32,
    original_usage: vk::ImageUsageFlags,
    max_image_count: Option<u32>,
) -> SwapchainMutation {
    let mut modified_usage = original_usage;
    let mut modified_min_image_count = original_min_image_count;

    if matches!(
        mode,
        Mode::ClearTest | Mode::CopyTest | Mode::HistoryCopyTest
    ) {
        modified_usage |= vk::ImageUsageFlags::TRANSFER_DST;
        let image_bump = if matches!(mode, Mode::CopyTest | Mode::HistoryCopyTest) {
            modified_usage |= vk::ImageUsageFlags::TRANSFER_SRC;
            2
        } else {
            1
        };

        let desired = original_min_image_count.saturating_add(image_bump);
        modified_min_image_count = match max_image_count {
            Some(max) if max > 0 => desired.min(max),
            _ => desired,
        };
    }

    if matches!(
        mode,
        Mode::BlendTest | Mode::AdaptiveBlendTest | Mode::SearchBlendTest
    ) {
        modified_usage |= vk::ImageUsageFlags::TRANSFER_SRC;
        modified_usage |= vk::ImageUsageFlags::SAMPLED;
        let desired = original_min_image_count.saturating_add(2);
        modified_min_image_count = match max_image_count {
            Some(max) if max > 0 => desired.min(max),
            _ => desired,
        };
    }

    if matches!(mode, Mode::MultiBlendTest | Mode::AdaptiveMultiBlendTest) {
        modified_usage |= vk::ImageUsageFlags::TRANSFER_SRC;
        modified_usage |= vk::ImageUsageFlags::SAMPLED;
        let desired = original_min_image_count.saturating_add(3);
        modified_min_image_count = match max_image_count {
            Some(max) if max > 0 => desired.min(max),
            _ => desired,
        };
    }

    SwapchainMutation {
        modified_min_image_count,
        modified_usage,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SimulatedPresentState {
    pub history_valid: bool,
    pub injection_works: bool,
    pub generated_present_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentSequence {
    PassThrough,
    OriginalThenGenerated,
    PrimeHistory,
    GeneratedThenOriginal,
}

pub fn planned_sequence(mode: Mode, state: &SimulatedPresentState) -> PresentSequence {
    match mode {
        Mode::PassThrough => PresentSequence::PassThrough,
        Mode::ClearTest | Mode::CopyTest => PresentSequence::OriginalThenGenerated,
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest
        | Mode::MultiBlendTest
        | Mode::AdaptiveMultiBlendTest
            if !state.history_valid =>
        {
            PresentSequence::PrimeHistory
        }
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest
        | Mode::MultiBlendTest
        | Mode::AdaptiveMultiBlendTest => PresentSequence::GeneratedThenOriginal,
    }
}

pub fn determine_adaptive_generated_frame_count(
    last_present_interval_ms: Option<f32>,
    threshold_ms: f32,
    min_count: u32,
    max_count: u32,
) -> u32 {
    let min_count = min_count.max(1);
    let max_count = max_count.max(min_count);
    let threshold_ms = threshold_ms.max(0.001);

    let Some(interval_ms) = last_present_interval_ms else {
        return max_count;
    };

    let scaled = (interval_ms / threshold_ms).floor() as u32;
    scaled.clamp(min_count, max_count)
}

pub fn mark_injection_result(
    mode: Mode,
    state: &mut SimulatedPresentState,
    injected_successfully: bool,
) {
    match mode {
        Mode::PassThrough => {}
        Mode::ClearTest | Mode::CopyTest => {
            if injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 1;
            }
        }
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest => {
            if state.history_valid && injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 1;
            }
            state.history_valid = true;
        }
        Mode::MultiBlendTest | Mode::AdaptiveMultiBlendTest => {
            if state.history_valid && injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 2;
            }
            state.history_valid = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        determine_adaptive_generated_frame_count, mark_injection_result, mutate_swapchain,
        planned_sequence, PresentSequence, SimulatedPresentState,
    };
    use crate::config::Mode;
    use ash::vk;

    #[test]
    fn passthrough_does_not_mutate_swapchain() {
        let result = mutate_swapchain(
            Mode::PassThrough,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(5),
        );
        assert_eq!(result.modified_min_image_count, 3);
        assert_eq!(result.modified_usage, vk::ImageUsageFlags::COLOR_ATTACHMENT);
    }

    #[test]
    fn clear_mode_adds_transfer_dst_and_one_extra_image() {
        let result = mutate_swapchain(
            Mode::ClearTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 4);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_DST));
        assert!(!result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
    }

    #[test]
    fn copy_mode_adds_transfer_src_dst_and_two_extra_images() {
        let result = mutate_swapchain(
            Mode::CopyTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 5);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_DST));
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
    }

    #[test]
    fn mutation_respects_surface_cap_maximum() {
        let result = mutate_swapchain(
            Mode::HistoryCopyTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(4),
        );
        assert_eq!(result.modified_min_image_count, 4);
    }

    #[test]
    fn blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::BlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 5);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
        assert!(!result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_DST));
    }

    #[test]
    fn adaptive_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::AdaptiveBlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 5);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
        assert!(!result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_DST));
    }

    #[test]
    fn search_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::SearchBlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 5);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
    }

    #[test]
    fn multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::MultiBlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 6);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
    }

    #[test]
    fn adaptive_multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::AdaptiveMultiBlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(10),
        );
        assert_eq!(result.modified_min_image_count, 6);
        assert!(result
            .modified_usage
            .contains(vk::ImageUsageFlags::TRANSFER_SRC));
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
    }

    #[test]
    fn history_copy_primes_then_switches_to_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::HistoryCopyTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::HistoryCopyTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::HistoryCopyTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::HistoryCopyTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::BlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::BlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::BlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::BlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn adaptive_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::AdaptiveBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::AdaptiveBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::AdaptiveBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::AdaptiveBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn search_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::SearchBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::SearchBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::SearchBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::SearchBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn multi_blend_mode_counts_two_generated_frames_per_real_frame() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::MultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::MultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::MultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::MultiBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 2);
        assert!(state.injection_works);
    }

    #[test]
    fn adaptive_multi_blend_mode_counts_two_generated_frames_per_real_frame() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::AdaptiveMultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::AdaptiveMultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::AdaptiveMultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::AdaptiveMultiBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 2);
        assert!(state.injection_works);
    }

    #[test]
    fn adaptive_generated_frame_count_scales_with_interval() {
        assert_eq!(determine_adaptive_generated_frame_count(None, 5.0, 1, 3), 3);
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(0.3), 5.0, 1, 3),
            1
        );
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(6.0), 5.0, 1, 3),
            1
        );
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(11.0), 5.0, 1, 3),
            2
        );
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(18.0), 5.0, 1, 3),
            3
        );
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(40.0), 5.0, 1, 3),
            3
        );
    }

    #[test]
    fn copy_and_clear_count_generated_frames() {
        let mut copy_state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::CopyTest, &copy_state),
            PresentSequence::OriginalThenGenerated
        );
        mark_injection_result(Mode::CopyTest, &mut copy_state, true);
        assert_eq!(copy_state.generated_present_count, 1);
        assert!(copy_state.injection_works);

        let mut clear_state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::ClearTest, &clear_state),
            PresentSequence::OriginalThenGenerated
        );
        mark_injection_result(Mode::ClearTest, &mut clear_state, true);
        assert_eq!(clear_state.generated_present_count, 1);
        assert!(clear_state.injection_works);
    }
}
