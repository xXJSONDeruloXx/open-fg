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
        Mode::ClearTest | Mode::BfiTest | Mode::CopyTest | Mode::HistoryCopyTest
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
        Mode::BlendTest
            | Mode::AdaptiveBlendTest
            | Mode::SearchBlendTest
            | Mode::SearchAdaptiveBlendTest
            | Mode::ReprojectBlendTest
            | Mode::ReprojectAdaptiveBlendTest
            | Mode::OptFlowBlendTest
            | Mode::OptFlowAdaptiveBlendTest
    ) {
        modified_usage |= vk::ImageUsageFlags::TRANSFER_SRC;
        modified_usage |= vk::ImageUsageFlags::SAMPLED;
        let desired = original_min_image_count.saturating_add(2);
        modified_min_image_count = match max_image_count {
            Some(max) if max > 0 => desired.min(max),
            _ => desired,
        };
    }

    if matches!(
        mode,
        Mode::MultiBlendTest
            | Mode::AdaptiveMultiBlendTest
            | Mode::ReprojectMultiBlendTest
            | Mode::ReprojectAdaptiveMultiBlendTest
            | Mode::OptFlowMultiBlendTest
            | Mode::OptFlowAdaptiveMultiBlendTest
    ) {
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
        Mode::ClearTest | Mode::BfiTest | Mode::CopyTest => PresentSequence::OriginalThenGenerated,
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest
        | Mode::SearchAdaptiveBlendTest
        | Mode::ReprojectBlendTest
        | Mode::ReprojectAdaptiveBlendTest
        | Mode::OptFlowBlendTest
        | Mode::OptFlowAdaptiveBlendTest
        | Mode::ReprojectMultiBlendTest
        | Mode::ReprojectAdaptiveMultiBlendTest
        | Mode::MultiBlendTest
        | Mode::AdaptiveMultiBlendTest
        | Mode::OptFlowMultiBlendTest
        | Mode::OptFlowAdaptiveMultiBlendTest
            if !state.history_valid =>
        {
            PresentSequence::PrimeHistory
        }
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest
        | Mode::SearchAdaptiveBlendTest
        | Mode::ReprojectBlendTest
        | Mode::ReprojectAdaptiveBlendTest
        | Mode::OptFlowBlendTest
        | Mode::OptFlowAdaptiveBlendTest
        | Mode::ReprojectMultiBlendTest
        | Mode::ReprojectAdaptiveMultiBlendTest
        | Mode::MultiBlendTest
        | Mode::AdaptiveMultiBlendTest
        | Mode::OptFlowMultiBlendTest
        | Mode::OptFlowAdaptiveMultiBlendTest => PresentSequence::GeneratedThenOriginal,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetFrameRateDecision {
    pub target_fps: f32,
    pub base_fps: f32,
    pub desired_generated_frames: f32,
    pub emitted_generated_frames: u32,
    pub next_credit: f32,
}

pub fn smooth_present_interval_ms(
    previous_interval_ms: Option<f32>,
    sample_interval_ms: Option<f32>,
    alpha: f32,
) -> Option<f32> {
    let alpha = alpha.clamp(0.0, 1.0);

    match (previous_interval_ms, sample_interval_ms) {
        (_, None) => previous_interval_ms,
        (None, Some(sample)) => Some(sample.max(0.0)),
        (Some(previous), Some(sample)) => {
            let previous = previous.max(0.0);
            let sample = sample.max(0.0);
            Some(previous + (sample - previous) * alpha)
        }
    }
}

pub fn determine_target_generated_frame_count(
    present_interval_ms: Option<f32>,
    target_fps: f32,
    min_count: u32,
    max_count: u32,
    prior_credit: f32,
) -> TargetFrameRateDecision {
    let min_count = min_count.min(max_count);
    let max_count = max_count.max(min_count);
    let target_fps = target_fps.max(0.0);

    let Some(interval_ms) = present_interval_ms.filter(|interval| *interval > 0.0) else {
        return TargetFrameRateDecision {
            target_fps,
            base_fps: 0.0,
            desired_generated_frames: min_count as f32,
            emitted_generated_frames: min_count,
            next_credit: 0.0,
        };
    };

    let base_fps = 1000.0 / interval_ms.max(0.001);
    let desired_generated_frames = ((target_fps / base_fps) - 1.0)
        .clamp(min_count as f32, max_count as f32)
        .max(0.0);

    if desired_generated_frames <= min_count as f32 + 1e-4 {
        return TargetFrameRateDecision {
            target_fps,
            base_fps,
            desired_generated_frames,
            emitted_generated_frames: min_count,
            next_credit: 0.0,
        };
    }

    let accumulated_credit =
        (prior_credit.max(0.0) + desired_generated_frames).min(max_count as f32 + 0.999_9);
    let emitted_generated_frames =
        ((accumulated_credit + 1e-4).floor() as u32).clamp(min_count, max_count);
    let next_credit = (accumulated_credit - emitted_generated_frames as f32).clamp(0.0, 0.999_9);

    TargetFrameRateDecision {
        target_fps,
        base_fps,
        desired_generated_frames,
        emitted_generated_frames,
        next_credit,
    }
}

pub fn mark_injection_result(
    mode: Mode,
    state: &mut SimulatedPresentState,
    injected_successfully: bool,
) {
    match mode {
        Mode::PassThrough => {}
        Mode::ClearTest | Mode::BfiTest | Mode::CopyTest => {
            if injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 1;
            }
        }
        Mode::HistoryCopyTest
        | Mode::BlendTest
        | Mode::AdaptiveBlendTest
        | Mode::SearchBlendTest
        | Mode::SearchAdaptiveBlendTest
        | Mode::ReprojectBlendTest
        | Mode::ReprojectAdaptiveBlendTest
        | Mode::OptFlowBlendTest
        | Mode::OptFlowAdaptiveBlendTest => {
            if state.history_valid && injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 1;
            }
            state.history_valid = true;
        }
        Mode::MultiBlendTest
        | Mode::AdaptiveMultiBlendTest
        | Mode::ReprojectMultiBlendTest
        | Mode::ReprojectAdaptiveMultiBlendTest
        | Mode::OptFlowMultiBlendTest
        | Mode::OptFlowAdaptiveMultiBlendTest => {
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
        determine_adaptive_generated_frame_count, determine_target_generated_frame_count,
        mark_injection_result, mutate_swapchain, planned_sequence, smooth_present_interval_ms,
        PresentSequence, SimulatedPresentState,
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
    fn bfi_mode_adds_transfer_dst_and_one_extra_image() {
        let result = mutate_swapchain(
            Mode::BfiTest,
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
    fn search_adaptive_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::SearchAdaptiveBlendTest,
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
    fn reproject_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::ReprojectBlendTest,
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
    fn reproject_adaptive_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::ReprojectAdaptiveBlendTest,
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
    fn optflow_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::OptFlowBlendTest,
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
    fn reproject_multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::ReprojectMultiBlendTest,
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
    fn reproject_adaptive_multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::ReprojectAdaptiveMultiBlendTest,
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
    fn search_adaptive_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::SearchAdaptiveBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::SearchAdaptiveBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::SearchAdaptiveBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::SearchAdaptiveBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn reproject_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::ReprojectBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::ReprojectBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::ReprojectBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::ReprojectBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn reproject_adaptive_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::ReprojectAdaptiveBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::ReprojectAdaptiveBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::ReprojectAdaptiveBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::ReprojectAdaptiveBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn optflow_blend_mode_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::OptFlowBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::OptFlowBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::OptFlowBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::OptFlowBlendTest, &mut state, true);
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
    fn reproject_multi_blend_mode_counts_two_generated_frames_per_real_frame() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::ReprojectMultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::ReprojectMultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::ReprojectMultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::ReprojectMultiBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 2);
        assert!(state.injection_works);
    }

    #[test]
    fn reproject_adaptive_multi_blend_mode_counts_two_generated_frames_per_real_frame() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::ReprojectAdaptiveMultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::ReprojectAdaptiveMultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::ReprojectAdaptiveMultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::ReprojectAdaptiveMultiBlendTest, &mut state, true);
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
    fn smooth_present_interval_blends_samples() {
        assert_eq!(smooth_present_interval_ms(None, None, 0.25), None);
        assert_eq!(
            smooth_present_interval_ms(None, Some(16.0), 0.25),
            Some(16.0)
        );
        assert_eq!(
            smooth_present_interval_ms(Some(20.0), Some(10.0), 0.25),
            Some(17.5)
        );
        assert_eq!(
            smooth_present_interval_ms(Some(20.0), Some(10.0), 1.0),
            Some(10.0)
        );
    }

    #[test]
    fn target_generated_frame_count_hits_integer_multiplier_exactly() {
        let decision =
            determine_target_generated_frame_count(Some(1000.0 / 60.0), 120.0, 0, 2, 0.0);
        assert_eq!(decision.emitted_generated_frames, 1);
        assert!((decision.desired_generated_frames - 1.0).abs() < 0.01);
        assert!(decision.next_credit < 0.01);
    }

    #[test]
    fn target_generated_frame_count_accumulates_fractional_credit() {
        let mut carry = 0.0;
        let mut emitted = Vec::new();
        for _ in 0..6 {
            let decision =
                determine_target_generated_frame_count(Some(1000.0 / 60.0), 100.0, 0, 2, carry);
            emitted.push(decision.emitted_generated_frames);
            carry = decision.next_credit;
        }
        assert_eq!(emitted, vec![0, 1, 1, 0, 1, 1]);
    }

    #[test]
    fn target_generated_frame_count_resets_credit_when_base_exceeds_target() {
        let decision =
            determine_target_generated_frame_count(Some(1000.0 / 144.0), 120.0, 0, 2, 0.8);
        assert_eq!(decision.emitted_generated_frames, 0);
        assert_eq!(decision.next_credit, 0.0);
    }

    #[test]
    fn target_generated_frame_count_can_alternate_between_one_and_two_generated_frames() {
        let mut carry = 0.0;
        let mut emitted = Vec::new();
        for _ in 0..4 {
            let decision =
                determine_target_generated_frame_count(Some(1000.0 / 60.0), 150.0, 0, 2, carry);
            emitted.push(decision.emitted_generated_frames);
            carry = decision.next_credit;
        }
        assert_eq!(emitted, vec![1, 2, 1, 2]);
    }

    #[test]
    fn target_generated_frame_count_clamps_to_maximum() {
        let decision = determine_target_generated_frame_count(Some(25.0), 180.0, 0, 2, 0.0);
        assert_eq!(decision.emitted_generated_frames, 2);
        assert!(decision.desired_generated_frames >= 2.0);
        assert!(decision.next_credit < 0.01);
    }

    #[test]
    fn optflow_adaptive_blend_mode_adds_sampled_and_transfer_src_usage() {
        let result = mutate_swapchain(
            Mode::OptFlowAdaptiveBlendTest,
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
    fn optflow_multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::OptFlowMultiBlendTest,
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
    fn optflow_adaptive_blend_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::OptFlowAdaptiveBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::OptFlowAdaptiveBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::OptFlowAdaptiveBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::OptFlowAdaptiveBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 1);
        assert!(state.injection_works);
    }

    #[test]
    fn optflow_multi_blend_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::OptFlowMultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::OptFlowMultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::OptFlowMultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::OptFlowMultiBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 2);
        assert!(state.injection_works);
    }

    #[test]
    fn optflow_adaptive_multi_blend_mode_requests_extra_headroom() {
        let result = mutate_swapchain(
            Mode::OptFlowAdaptiveMultiBlendTest,
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
    fn optflow_adaptive_multi_blend_uses_history_prime_then_generated_before_original() {
        let mut state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::OptFlowAdaptiveMultiBlendTest, &state),
            PresentSequence::PrimeHistory
        );
        mark_injection_result(Mode::OptFlowAdaptiveMultiBlendTest, &mut state, true);
        assert!(state.history_valid);
        assert_eq!(state.generated_present_count, 0);
        assert_eq!(
            planned_sequence(Mode::OptFlowAdaptiveMultiBlendTest, &state),
            PresentSequence::GeneratedThenOriginal
        );
        mark_injection_result(Mode::OptFlowAdaptiveMultiBlendTest, &mut state, true);
        assert_eq!(state.generated_present_count, 2);
        assert!(state.injection_works);
    }

    #[test]
    fn copy_clear_and_bfi_count_generated_frames() {
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

        let mut bfi_state = SimulatedPresentState::default();
        assert_eq!(
            planned_sequence(Mode::BfiTest, &bfi_state),
            PresentSequence::OriginalThenGenerated
        );
        mark_injection_result(Mode::BfiTest, &mut bfi_state, true);
        assert_eq!(bfi_state.generated_present_count, 1);
        assert!(bfi_state.injection_works);
    }

    // ---- mutate_swapchain: no cap (None) and zero cap (Some(0)) ----

    #[test]
    fn clear_mode_with_no_cap_expands_freely() {
        let result = mutate_swapchain(
            Mode::ClearTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            None,
        );
        assert_eq!(result.modified_min_image_count, 4);
        assert!(result.modified_usage.contains(vk::ImageUsageFlags::TRANSFER_DST));
    }

    #[test]
    fn bfi_mode_with_no_cap_expands_freely() {
        let result = mutate_swapchain(
            Mode::BfiTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            None,
        );
        assert_eq!(result.modified_min_image_count, 4);
    }

    #[test]
    fn copy_mode_with_no_cap_expands_freely() {
        let result = mutate_swapchain(
            Mode::CopyTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            None,
        );
        assert_eq!(result.modified_min_image_count, 5);
    }

    #[test]
    fn history_copy_with_no_cap_expands_freely() {
        let result = mutate_swapchain(
            Mode::HistoryCopyTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            None,
        );
        assert_eq!(result.modified_min_image_count, 5);
    }

    #[test]
    fn blend_modes_with_no_cap_expand_freely() {
        for mode in [
            Mode::BlendTest,
            Mode::AdaptiveBlendTest,
            Mode::SearchBlendTest,
            Mode::SearchAdaptiveBlendTest,
            Mode::ReprojectBlendTest,
            Mode::ReprojectAdaptiveBlendTest,
            Mode::OptFlowBlendTest,
            Mode::OptFlowAdaptiveBlendTest,
        ] {
            let result = mutate_swapchain(mode, 3, vk::ImageUsageFlags::COLOR_ATTACHMENT, None);
            assert_eq!(result.modified_min_image_count, 5, "mode {:?}", mode);
            assert!(result.modified_usage.contains(vk::ImageUsageFlags::TRANSFER_SRC));
            assert!(result.modified_usage.contains(vk::ImageUsageFlags::SAMPLED));
        }
    }

    #[test]
    fn multi_modes_with_no_cap_expand_freely() {
        for mode in [
            Mode::MultiBlendTest,
            Mode::AdaptiveMultiBlendTest,
            Mode::ReprojectMultiBlendTest,
            Mode::ReprojectAdaptiveMultiBlendTest,
            Mode::OptFlowMultiBlendTest,
            Mode::OptFlowAdaptiveMultiBlendTest,
        ] {
            let result = mutate_swapchain(mode, 3, vk::ImageUsageFlags::COLOR_ATTACHMENT, None);
            assert_eq!(result.modified_min_image_count, 6, "mode {:?}", mode);
        }
    }

    #[test]
    fn zero_max_image_count_treated_as_uncapped() {
        // Some(0) means the surface has no explicit upper limit — treat as uncapped.
        let result = mutate_swapchain(
            Mode::BlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(0),
        );
        assert_eq!(result.modified_min_image_count, 5);
    }

    #[test]
    fn multi_zero_max_image_count_treated_as_uncapped() {
        let result = mutate_swapchain(
            Mode::MultiBlendTest,
            3,
            vk::ImageUsageFlags::COLOR_ATTACHMENT,
            Some(0),
        );
        assert_eq!(result.modified_min_image_count, 6);
    }

    // ---- mark_injection_result: failed injection does not advance counts ----

    #[test]
    fn passthrough_mark_injection_result_never_mutates_state() {
        let mut state = SimulatedPresentState::default();
        mark_injection_result(Mode::PassThrough, &mut state, true);
        assert_eq!(state.generated_present_count, 0);
        assert!(!state.injection_works);
        assert!(!state.history_valid);
        mark_injection_result(Mode::PassThrough, &mut state, false);
        assert_eq!(state.generated_present_count, 0);
        assert!(!state.injection_works);
        assert!(!state.history_valid);
    }

    #[test]
    fn copy_clear_bfi_failed_injection_does_not_count() {
        for mode in [Mode::CopyTest, Mode::ClearTest, Mode::BfiTest] {
            let mut state = SimulatedPresentState::default();
            mark_injection_result(mode, &mut state, false);
            assert_eq!(state.generated_present_count, 0, "mode {:?}", mode);
            assert!(!state.injection_works, "mode {:?}", mode);
        }
    }

    #[test]
    fn blend_failed_injection_after_prime_does_not_count() {
        for mode in [
            Mode::BlendTest,
            Mode::AdaptiveBlendTest,
            Mode::SearchBlendTest,
            Mode::SearchAdaptiveBlendTest,
            Mode::ReprojectBlendTest,
            Mode::ReprojectAdaptiveBlendTest,
            Mode::OptFlowBlendTest,
            Mode::OptFlowAdaptiveBlendTest,
        ] {
            let mut state = SimulatedPresentState::default();
            // prime history
            mark_injection_result(mode, &mut state, true);
            assert!(state.history_valid, "mode {:?}", mode);
            assert_eq!(state.generated_present_count, 0, "mode {:?}", mode);
            // failed injection while history valid
            mark_injection_result(mode, &mut state, false);
            assert_eq!(
                state.generated_present_count, 0,
                "mode {:?} failed inject should not count",
                mode
            );
            assert!(
                !state.injection_works,
                "mode {:?} injection_works should stay false after failure",
                mode
            );
        }
    }

    #[test]
    fn multi_failed_injection_after_prime_does_not_count() {
        for mode in [
            Mode::MultiBlendTest,
            Mode::AdaptiveMultiBlendTest,
            Mode::ReprojectMultiBlendTest,
            Mode::ReprojectAdaptiveMultiBlendTest,
            Mode::OptFlowMultiBlendTest,
            Mode::OptFlowAdaptiveMultiBlendTest,
        ] {
            let mut state = SimulatedPresentState::default();
            mark_injection_result(mode, &mut state, true);
            assert!(state.history_valid, "mode {:?}", mode);
            mark_injection_result(mode, &mut state, false);
            assert_eq!(
                state.generated_present_count, 0,
                "mode {:?} failed inject should not count",
                mode
            );
            assert!(
                !state.injection_works,
                "mode {:?} injection_works should stay false after failure",
                mode
            );
        }
    }

    // ---- determine_target_generated_frame_count edge cases ----

    #[test]
    fn target_generated_frame_count_with_no_interval_falls_back_to_min_count() {
        let decision = determine_target_generated_frame_count(None, 120.0, 0, 2, 0.0);
        assert_eq!(decision.emitted_generated_frames, 0);
        assert_eq!(decision.base_fps, 0.0);

        let decision_min1 = determine_target_generated_frame_count(None, 120.0, 1, 3, 0.5);
        assert_eq!(decision_min1.emitted_generated_frames, 1);
    }

    #[test]
    fn target_generated_frame_count_enforces_min_count() {
        // Base is 60 fps, target 80 fps — desired ~0.33 generated frames.
        // With min_count=1, should still emit 1.
        let decision =
            determine_target_generated_frame_count(Some(1000.0 / 60.0), 80.0, 1, 3, 0.0);
        assert_eq!(decision.emitted_generated_frames, 1);
    }

    // ---- determine_adaptive_generated_frame_count edge cases ----

    #[test]
    fn adaptive_frame_count_when_min_equals_max_always_returns_that_value() {
        for interval in [None, Some(1.0), Some(5.0), Some(100.0)] {
            assert_eq!(
                determine_adaptive_generated_frame_count(interval, 5.0, 2, 2),
                2,
                "interval {:?}",
                interval
            );
        }
    }

    #[test]
    fn adaptive_frame_count_with_zero_threshold_clamps_high() {
        // threshold_ms = 0.001 minimum applied internally
        assert_eq!(
            determine_adaptive_generated_frame_count(Some(50.0), 0.0, 1, 4),
            4
        );
    }

    // ---- smooth_present_interval edge cases ----

    #[test]
    fn smooth_present_interval_with_zero_alpha_ignores_new_sample() {
        assert_eq!(
            smooth_present_interval_ms(Some(20.0), Some(10.0), 0.0),
            Some(20.0)
        );
    }

    #[test]
    fn smooth_present_interval_clamps_negative_alpha() {
        // Negative alpha should behave as 0.0 (clamped)
        assert_eq!(
            smooth_present_interval_ms(Some(20.0), Some(10.0), -1.0),
            Some(20.0)
        );
    }
}
