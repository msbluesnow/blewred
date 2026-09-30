use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub score: f32,
    pub class_id: usize,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct TrackedObject {
    pub id: u64,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    /// Velocity vector (dx1, dy1, dx2, dy2) for inertial motion continuation
    pub velocity: (f32, f32, f32, f32),
    pub score: f32,
    pub class_id: usize,
    pub label: String,
    pub hits: usize,
    pub time_since_update: usize,
}

#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Percentage by which bounding boxes are dilated outward (0.18 = +18%)
    pub dilation_rate: f32,
    /// IoU matching threshold for multi-object association
    pub iou_threshold: f32,
    /// Hold-time retention: number of frames a box is kept along motion inertia (30 frames ~ 0.5s at 60 FPS)
    pub max_age: usize,
    /// Velocity damping factor per predicted frame
    pub velocity_damping: f32,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            dilation_rate: 0.18,
            iou_threshold: 0.25,
            max_age: 25,
            velocity_damping: 0.92,
        }
    }
}

pub struct ZeroMissTracker {
    config: TrackerConfig,
    tracks: Vec<TrackedObject>,
    next_id: u64,
}

impl ZeroMissTracker {
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            config,
            tracks: Vec::new(),
            next_id: 1,
        }
    }

    /// Predict next positions of all tracks using motion vector inertia
    pub fn predict(&mut self) {
        for trk in &mut self.tracks {
            // Apply inertial velocity extrapolation
            let (dx1, dy1, dx2, dy2) = trk.velocity;
            trk.x1 = (trk.x1 + dx1).clamp(0.0, 1.0);
            trk.y1 = (trk.y1 + dy1).clamp(0.0, 1.0);
            trk.x2 = (trk.x2 + dx2).clamp(0.0, 1.0);
            trk.y2 = (trk.y2 + dy2).clamp(0.0, 1.0);

            // Dampen velocity when coasting on inertia without fresh detection
            trk.velocity.0 *= self.config.velocity_damping;
            trk.velocity.1 *= self.config.velocity_damping;
            trk.velocity.2 *= self.config.velocity_damping;
            trk.velocity.3 *= self.config.velocity_damping;
        }
    }

    /// Calculate IoU between two bounding boxes
    fn calculate_iou(
        b1: (f32, f32, f32, f32),
        b2: (f32, f32, f32, f32),
    ) -> f32 {
        let x1 = b1.0.max(b2.0);
        let y1 = b1.1.max(b2.1);
        let x2 = b1.2.min(b2.2);
        let y2 = b1.3.min(b2.3);

        let inter_w = (x2 - x1).max(0.0);
        let inter_h = (y2 - y1).max(0.0);
        let inter_area = inter_w * inter_h;

        let a1 = (b1.2 - b1.0).max(0.0) * (b1.3 - b1.1).max(0.0);
        let a2 = (b2.2 - b2.0).max(0.0) * (b2.3 - b2.1).max(0.0);
        let union_area = a1 + a2 - inter_area;

        if union_area > 0.0 {
            inter_area / union_area
        } else {
            0.0
        }
    }

    /// Associate detections with tracks, update kinematics and maintain hysteresis
    pub fn update(&mut self, detections: &[Detection]) {
        self.predict();

        let num_tracks = self.tracks.len();
        let num_dets = detections.len();

        let mut matched_tracks = vec![false; num_tracks];
        let mut matched_dets = vec![false; num_dets];

        // Greedy matching by maximum IoU
        if num_tracks > 0 && num_dets > 0 {
            let mut matches = Vec::new();
            for (t_idx, trk) in self.tracks.iter().enumerate() {
                for (d_idx, det) in detections.iter().enumerate() {
                    // Match preferably same class or high overlap
                    let class_match = trk.class_id == det.class_id;
                    let iou = Self::calculate_iou(
                        (trk.x1, trk.y1, trk.x2, trk.y2),
                        (det.x1, det.y1, det.x2, det.y2),
                    );

                    let thresh = if class_match {
                        self.config.iou_threshold
                    } else {
                        self.config.iou_threshold + 0.15
                    };

                    if iou >= thresh {
                        matches.push((iou, t_idx, d_idx));
                    }
                }
            }

            // Sort matches by IoU descending
            matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            for (_iou, t_idx, d_idx) in matches {
                if !matched_tracks[t_idx] && !matched_dets[d_idx] {
                    matched_tracks[t_idx] = true;
                    matched_dets[d_idx] = true;

                    let det = &detections[d_idx];
                    let trk = &mut self.tracks[t_idx];

                    // Calculate instantaneous velocity vector
                    let new_dx1 = det.x1 - trk.x1;
                    let new_dy1 = det.y1 - trk.y1;
                    let new_dx2 = det.x2 - trk.x2;
                    let new_dy2 = det.y2 - trk.y2;

                    // Smooth velocity with alpha filter (Kalman approximation)
                    const ALPHA_V: f32 = 0.55;
                    trk.velocity.0 = ALPHA_V * new_dx1 + (1.0 - ALPHA_V) * trk.velocity.0;
                    trk.velocity.1 = ALPHA_V * new_dy1 + (1.0 - ALPHA_V) * trk.velocity.1;
                    trk.velocity.2 = ALPHA_V * new_dx2 + (1.0 - ALPHA_V) * trk.velocity.2;
                    trk.velocity.3 = ALPHA_V * new_dy2 + (1.0 - ALPHA_V) * trk.velocity.3;

                    // Smooth coordinates to eliminate detector jitter
                    const ALPHA_POS: f32 = 0.85;
                    trk.x1 = ALPHA_POS * det.x1 + (1.0 - ALPHA_POS) * trk.x1;
                    trk.y1 = ALPHA_POS * det.y1 + (1.0 - ALPHA_POS) * trk.y1;
                    trk.x2 = ALPHA_POS * det.x2 + (1.0 - ALPHA_POS) * trk.x2;
                    trk.y2 = ALPHA_POS * det.y2 + (1.0 - ALPHA_POS) * trk.y2;

                    trk.score = det.score;
                    trk.class_id = det.class_id;
                    trk.label = det.label.clone();
                    trk.hits += 1;
                    trk.time_since_update = 0;
                }
            }
        }

        // Handle unmatched detections -> create new tracks
        for (d_idx, matched) in matched_dets.iter().enumerate() {
            if !matched {
                let det = &detections[d_idx];
                self.tracks.push(TrackedObject {
                    id: self.next_id,
                    x1: det.x1,
                    y1: det.y1,
                    x2: det.x2,
                    y2: det.y2,
                    velocity: (0.0, 0.0, 0.0, 0.0),
                    score: det.score,
                    class_id: det.class_id,
                    label: det.label.clone(),
                    hits: 1,
                    time_since_update: 0,
                });
                self.next_id += 1;
            }
        }

        // Handle unmatched tracks -> age and maintain hysteresis
        for (t_idx, matched) in matched_tracks.iter().enumerate() {
            if !matched {
                self.tracks[t_idx].time_since_update += 1;
                // Gradually decay confidence during hold retention
                self.tracks[t_idx].score *= 0.96;
            }
        }

        // Purge tracks that exceed max_age (30 frames)
        let max_age = self.config.max_age;
        self.tracks.retain(|trk| trk.time_since_update <= max_age);
    }

    /// Retrieve active and hold-retained boxes with Bounding Box Dilation (+15-20%)
    /// To ensure complete coverage with zero edge pixel leaks
    pub fn get_dilated_boxes(&self) -> Vec<crate::cascade::NormalizedBox> {
        let mut result = Vec::with_capacity(self.tracks.len());
        let half_dil = self.config.dilation_rate / 2.0;

        for trk in &self.tracks {
            let w = (trk.x2 - trk.x1).max(0.01);
            let h = (trk.y2 - trk.y1).max(0.01);

            let dx = w * half_dil;
            let dy = h * half_dil;

            // Expand outward symmetrically and clamp to screen bounds
            let dilated_x1 = (trk.x1 - dx).clamp(0.0, 1.0);
            let dilated_y1 = (trk.y1 - dy).clamp(0.0, 1.0);
            let dilated_x2 = (trk.x2 + dx).clamp(0.0, 1.0);
            let dilated_y2 = (trk.y2 + dy).clamp(0.0, 1.0);

            result.push(crate::cascade::NormalizedBox {
                x1: dilated_x1,
                y1: dilated_y1,
                x2: dilated_x2,
                y2: dilated_y2,
                class_id: trk.class_id,
                label: trk.label.clone(),
                score: trk.score,
            });
        }

        result
    }

    /// Check if tracker has any active tracks (currently visible or in hold retention)
    pub fn is_active(&self) -> bool {
        !self.tracks.is_empty()
    }

    /// Count of active tracks
    pub fn active_track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Clear all tracks (e.g. on scene switch or user reset)
    pub fn reset(&mut self) {
        self.tracks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_dilation_and_inertia() {
        let config = TrackerConfig {
            dilation_rate: 0.20,
            iou_threshold: 0.25,
            max_age: 30,
            velocity_damping: 0.90,
        };

        let mut tracker = ZeroMissTracker::new(config);

        let det1 = vec![Detection {
            x1: 0.40,
            y1: 0.40,
            x2: 0.60,
            y2: 0.60,
            score: 0.95,
            class_id: 3,
            label: "FEMALE_BREAST_EXPOSED".to_string(),
        }];

        tracker.update(&det1);
        assert_eq!(tracker.active_track_count(), 1);

        let dilated = tracker.get_dilated_boxes();
        assert_eq!(dilated.len(), 1);
        assert!((dilated[0].x1 - 0.38).abs() < 1e-3);
        assert!((dilated[0].y1 - 0.38).abs() < 1e-3);
        assert!((dilated[0].x2 - 0.62).abs() < 1e-3);
        assert!((dilated[0].y2 - 0.62).abs() < 1e-3);

        let det2 = vec![Detection {
            x1: 0.42,
            y1: 0.40,
            x2: 0.62,
            y2: 0.60,
            score: 0.94,
            class_id: 3,
            label: "FEMALE_BREAST_EXPOSED".to_string(),
        }];
        tracker.update(&det2);

        let empty_dets = vec![];
        for _ in 3..=7 {
            tracker.update(&empty_dets);
            let boxes = tracker.get_dilated_boxes();
            assert_eq!(boxes.len(), 1);
            assert!(boxes[0].x1 > 0.38);
        }

        for _ in 0..25 {
            tracker.update(&empty_dets);
        }
        assert!(tracker.is_active());

        tracker.update(&empty_dets);
        assert_eq!(tracker.active_track_count(), 0);
        assert!(!tracker.is_active());
    }
}
