#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymmetricSearchConfig {
    pub search_radius: i32,
    pub patch_radius: i32,
    pub confidence_scale: f32,
    pub motion_penalty: f32,
}

impl Default for SymmetricSearchConfig {
    fn default() -> Self {
        Self {
            search_radius: 2,
            patch_radius: 1,
            confidence_scale: 0.5,
            motion_penalty: 0.01,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymmetricSearchResult {
    pub offset_x: i32,
    pub offset_y: i32,
    pub best_error: f32,
    pub zero_error: f32,
    pub confidence: f32,
}

fn clamp_coord(value: i32, limit: usize) -> usize {
    value.clamp(0, limit.saturating_sub(1) as i32) as usize
}

fn sample(frame: &[f32], width: usize, height: usize, x: i32, y: i32) -> f32 {
    let xi = clamp_coord(x, width);
    let yi = clamp_coord(y, height);
    frame[yi * width + xi]
}

fn symmetric_patch_error(
    prev: &[f32],
    curr: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    offset_x: i32,
    offset_y: i32,
    patch_radius: i32,
) -> f32 {
    let mut error = 0.0;
    let x = x as i32;
    let y = y as i32;

    for patch_y in -patch_radius..=patch_radius {
        for patch_x in -patch_radius..=patch_radius {
            let prev_sample = sample(
                prev,
                width,
                height,
                x + offset_x + patch_x,
                y + offset_y + patch_y,
            );
            let curr_sample = sample(
                curr,
                width,
                height,
                x - offset_x + patch_x,
                y - offset_y + patch_y,
            );
            error += (prev_sample - curr_sample).abs();
        }
    }

    error
}

pub fn estimate_symmetric_motion_offset(
    prev: &[f32],
    curr: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    config: SymmetricSearchConfig,
) -> SymmetricSearchResult {
    assert_eq!(prev.len(), width * height);
    assert_eq!(curr.len(), width * height);

    let search_radius = config.search_radius.max(0);
    let patch_radius = config.patch_radius.max(0);

    let zero_error = symmetric_patch_error(prev, curr, width, height, x, y, 0, 0, patch_radius);
    let mut best_error = zero_error;
    let mut best_offset = (0, 0);

    for offset_y in -search_radius..=search_radius {
        for offset_x in -search_radius..=search_radius {
            let motion_cost =
                config.motion_penalty * (offset_x * offset_x + offset_y * offset_y) as f32;
            let error = symmetric_patch_error(
                prev,
                curr,
                width,
                height,
                x,
                y,
                offset_x,
                offset_y,
                patch_radius,
            ) + motion_cost;

            if error < best_error {
                best_error = error;
                best_offset = (offset_x, offset_y);
            }
        }
    }

    let confidence = ((zero_error - best_error) * config.confidence_scale).clamp(0.0, 1.0);

    SymmetricSearchResult {
        offset_x: best_offset.0,
        offset_y: best_offset.1,
        best_error,
        zero_error,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::{estimate_symmetric_motion_offset, SymmetricSearchConfig};

    #[test]
    fn identical_frames_fall_back_to_zero_motion() {
        let prev = vec![0.0, 0.2, 0.4, 0.6, 0.8];
        let curr = prev.clone();
        let result = estimate_symmetric_motion_offset(
            &prev,
            &curr,
            5,
            1,
            2,
            0,
            SymmetricSearchConfig::default(),
        );
        assert_eq!((result.offset_x, result.offset_y), (0, 0));
        assert_eq!(result.best_error, 0.0);
        assert_eq!(result.zero_error, 0.0);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn finds_expected_half_motion_offset_on_shifted_gradient() {
        let prev = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let curr = vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0];
        let result = estimate_symmetric_motion_offset(
            &prev,
            &curr,
            7,
            1,
            3,
            0,
            SymmetricSearchConfig {
                search_radius: 2,
                patch_radius: 0,
                confidence_scale: 0.5,
                motion_penalty: 0.01,
            },
        );
        assert_eq!((result.offset_x, result.offset_y), (-1, 0));
        assert!(result.best_error < result.zero_error);
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn flat_regions_keep_low_confidence_even_with_search_radius() {
        let prev = vec![0.5; 25];
        let curr = vec![0.5; 25];
        let result = estimate_symmetric_motion_offset(
            &prev,
            &curr,
            5,
            5,
            2,
            2,
            SymmetricSearchConfig::default(),
        );
        assert_eq!((result.offset_x, result.offset_y), (0, 0));
        assert_eq!(result.confidence, 0.0);
    }
}
