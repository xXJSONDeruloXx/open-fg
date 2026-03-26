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
        Mode::HistoryCopyTest if !state.history_valid => PresentSequence::PrimeHistory,
        Mode::HistoryCopyTest => PresentSequence::GeneratedThenOriginal,
    }
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
        Mode::HistoryCopyTest => {
            if state.history_valid && injected_successfully {
                state.injection_works = true;
                state.generated_present_count += 1;
            }
            state.history_valid = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        mark_injection_result, mutate_swapchain, planned_sequence, PresentSequence,
        SimulatedPresentState,
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
