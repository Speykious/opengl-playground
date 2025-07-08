use glam::{vec2, Vec2};

use crate::scenes::osu_slider::path_approx::{self, CATMULL_SEGMENT_LENGTH, CIRCULAR_ARC_TOLERANCE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SliderCurveType {
    Inherit,
    Bezier,
    Catmull,
    Linear,
    PerfectCircle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderControlPoint {
    pub curve_type: SliderCurveType,
    pub x: f32,
    pub y: f32,
}

impl SliderControlPoint {
	pub const fn new(curve_type: SliderCurveType, x: f32, y: f32) -> Self {
		Self { curve_type, x, y }
	}
}

impl SliderControlPoint {
    pub fn vec2(&self) -> Vec2 {
        vec2(self.x, self.y)
    }
}

pub struct SliderPath {
    pub control_points: Vec<SliderControlPoint>,
    pub length: f64,
}

#[derive(Default)]
pub struct CalculatedPath {
    pub points: Vec<Vec2>,
    pub segment_ends: Vec<usize>,
}

pub struct CalculatedLength {
    pub cumulative_length: Vec<f64>,
    pub segment_end_distances: Vec<f64>,
}

impl CalculatedLength {
    pub fn distance(&self) -> f64 {
        self.cumulative_length.last().copied().unwrap_or_default()
    }
}

impl SliderPath {
    pub fn path_to_progress(
        calculated_path: &CalculatedPath,
        calculated_length: &CalculatedLength,
        p0: f64,
        p1: f64,
    ) -> Vec<Vec2> {
        let mut path = Vec::new();

        let distance = calculated_length.distance();

        let d0 = p0.clamp(0.0, 1.0) * distance;
        let d1 = p1.clamp(0.0, 1.0) * distance;

        let mut i = 0;
        while i < calculated_path.points.len() && calculated_length.cumulative_length[i] < d0 {
            i += 1;
        }

        path.push(Self::interpolate_vertices(
            calculated_path,
            calculated_length,
            i,
            d0,
        ));

        while i < calculated_path.points.len() && calculated_length.cumulative_length[i] < d1 {
            path.push(calculated_path.points[i]);
            i += 1;
        }

        path.push(Self::interpolate_vertices(
            calculated_path,
            calculated_length,
            i,
            d1,
        ));

        path
    }

    fn interpolate_vertices(
        calculated_path: &CalculatedPath,
        calculated_length: &CalculatedLength,
        i: usize,
        d: f64,
    ) -> Vec2 {
        if calculated_path.points.is_empty() {
            return Vec2::ZERO;
        }

        if i == 0 {
            return calculated_path.points.first().copied().unwrap();
        }
        if i >= calculated_path.points.len() {
            return calculated_path.points.last().copied().unwrap();
        }

        let p0 = calculated_path.points[i - 1];
        let p1 = calculated_path.points[i];

        let d0 = calculated_length.cumulative_length[i - 1];
        let d1 = calculated_length.cumulative_length[i];

        // Avoid division by and almost-zero number in case two points are extremely close to each other.
        if (d0 - d1).abs() < 0.001 {
            return p0;
        }

        let w = (d - d0) / (d1 - d0);
        p0 + (p1 - p0) * w as f32
    }

    pub fn calculate_path_and_length(&self) -> Option<(CalculatedPath, CalculatedLength)> {
        if self.control_points.is_empty() {
            return None;
        }

        let mut calculated_path = Vec::new();
        let mut segment_ends = Vec::new();
		let mut optimized_length = None;

        let last_cpindex = self.control_points.len() - 1;

        let mut start = 0;
        for (i, control_point) in self.control_points.iter().enumerate() {
            if control_point.curve_type == SliderCurveType::Inherit && i < last_cpindex {
                continue;
            }

            // The current vertex ends the segment
            let segment_vertices = &self.control_points[start..=i];
            let segment_type = self.control_points[start].curve_type;

            // No need to calculate path when there is only 1 vertex
            if segment_vertices.len() == 1 {
                calculated_path.push(segment_vertices[0].vec2());
            } else if segment_vertices.len() > 1 {
                let (sub_path, optimized_sub_length) =
                    Self::calculate_sub_path(segment_vertices, segment_type, true);

				if let Some(optimized_sub_length) = optimized_sub_length {
					*optimized_length.get_or_insert_default() += optimized_sub_length;
				}

                // Skip the first vertex if it is the same as the last vertex from the previous segment
                let skip_first = !calculated_path.is_empty()
                    && !sub_path.is_empty()
                    && calculated_path.last() == Some(&sub_path[0]);
                let sub_path_start = if skip_first { 1 } else { 0 };

                calculated_path.extend_from_slice(&sub_path[sub_path_start..]);
            }

            if i > 0 {
                // Remember the index of the segment end
                segment_ends.push(calculated_path.len() - 1);
            }

            // Start the new segment at the current vertex
            start = i;
        }

        let mut path = CalculatedPath {
            points: calculated_path,
            segment_ends,
        };

		let length = Self::calculate_length(&mut path, optimized_length, None/*Some(self.length)*/);

		Some((path, length))
    }

    fn calculate_sub_path(
        segment_vertices: &[SliderControlPoint],
        segment_type: SliderCurveType,
        optimize_catmull: bool,
    ) -> (Vec<Vec2>, Option<f64>) {
        'curve_match: {
            match segment_type {
                SliderCurveType::Inherit | SliderCurveType::Linear => {
                    return (
                        path_approx::linear_to_piecewise_linear(segment_vertices),
                        None,
                    );
                }
                SliderCurveType::PerfectCircle => {
                    if segment_vertices.len() != 3 {
                        break 'curve_match;
                    }

                    // Revert to b-spline if the arc isn't valid
                    let Some(carps) = path_approx::circular_arc_props(segment_vertices) else {
                        break 'curve_match;
                    };

                    // taken from https://github.com/ppy/osu-framework/blob/1201e641699a1d50d2f6f9295192dad6263d5820/osu.Framework/Utils/PathApproximator.cs#L181-L186
                    let sub_points = match carps.radius as f64 * 2.0 <= CIRCULAR_ARC_TOLERANCE {
                        true => 2,
                        false => 2.max(
                            (carps.theta_range
                                / (2.0
                                    * (1.0 - (CIRCULAR_ARC_TOLERANCE / carps.radius as f64))
                                        .acos()))
                            .ceil() as usize,
                        ),
                    };

                    // 1000 subpoints requires an arc length of at least ~120 thousand to occur
                    // See here for calculations https://www.desmos.com/calculator/umj6jvmcz7
                    if sub_points >= 1000 {
                        break 'curve_match;
                    }

                    let sub_path = path_approx::circular_arc_to_piecewise_linear(carps);

                    if sub_path.is_empty() {
                        break 'curve_match;
                    }

                    return (sub_path, None);
                }
                SliderCurveType::Catmull => {
                    let sub_path = path_approx::catmull_to_piecewise_linear(segment_vertices);

                    if !optimize_catmull {
                        return (sub_path, None);
                    }

                    // At draw time, osu!stable optimises paths by only keeping piecewise segments that are 6px apart.
                    // For the most part we don't care about this optimisation, and its additional heuristics are hard to reproduce in every implementation.
                    //
                    // However, it matters for Catmull paths which form "bulbs" around sequential knots with identical positions,
                    // so we'll apply a very basic form of the optimisation here and return a length representing the optimised portion.
                    // The returned length is important so that the optimisation doesn't cause the path to get extended to match the value of ExpectedDistance.

                    let mut optimized_path = Vec::with_capacity(sub_path.len());
                    let mut optimized_length = 0.0;

                    let mut last_start_opt: Option<Vec2> = None;
                    let mut length_removed_since_start: f64 = 0.0;

                    for i in 0..sub_path.len() {
                        let Some(last_start) = last_start_opt else {
                            optimized_path.push(sub_path[i]);
                            last_start_opt = Some(sub_path[i]);
                            continue;
                        };

                        debug_assert!(i > 0);

                        let dist_from_start = last_start.distance(sub_path[i]) as f64;
                        length_removed_since_start += sub_path[i - 1].distance(sub_path[i]) as f64;

                        // Either 6px from the start, the last vertex at every knot, or the end of the path.
                        if dist_from_start > 6.0
                            || (i + 1) % CATMULL_SEGMENT_LENGTH == 0
                            || i == sub_path.len() - 1
                        {
                            optimized_path.push(sub_path[i]);
                            optimized_length += length_removed_since_start - dist_from_start;

                            last_start_opt = None;
                            length_removed_since_start = 0.0;
                        }
                    }

                    return (optimized_path, Some(optimized_length));
                }
                _ => {}
            }
        }

        (
            path_approx::bezier_to_piecewise_linear(segment_vertices),
            None,
        )
    }

    pub fn calculate_length(
        calculated_path: &mut CalculatedPath,
        optimized_length: Option<f64>,
        expected_distance: Option<f64>,
    ) -> CalculatedLength {
        let mut calculated_length = optimized_length.unwrap_or_default();
        let mut cumulative_length = vec![0.0];

        let cpth = &mut calculated_path.points;

        for i in 0..(cpth.len() - 1) {
            let diff = cpth[i + 1] - cpth[i];
            calculated_length += diff.length() as f64;
            cumulative_length.push(calculated_length);
        }

        // Store the distances of the segment ends now, because after shortening the indices may be out of range
        let segment_end_distances = (calculated_path.segment_ends.iter())
            .map(|&i| cumulative_length[i])
            .collect::<Vec<_>>();

        'expected_distance: {
            if let Some(expected_distance) = expected_distance {
                if calculated_length == expected_distance {
                    break 'expected_distance;
                }

                // In osu-stable, if the last two path points of a slider are equal, extension is not performed.
                if cpth.len() >= 2
                    && cpth[cpth.len() - 1] == cpth[cpth.len() - 2]
                    && expected_distance > calculated_length
                {
                    cumulative_length.push(calculated_length);
                    break 'expected_distance;
                }

                // The last length is always incorrect
                cumulative_length.pop();

                if calculated_length > expected_distance {
                    // The path will be shortened further, in which case we should trim any more unnecessary lengths and their associated path segments
                    while cumulative_length
                        .last()
                        .is_some_and(|&l| l >= expected_distance)
                    {
                        cumulative_length.pop();
                        cpth.pop();
                    }
                }

                let cpth_len = cpth.len();
                if cpth_len < 2 {
                    // The expected distance is negative or zero
                    cumulative_length.push(0.0);
                    break 'expected_distance;
                }

                // The direction of the segment to shorten or lengthen
                let dir = (cpth[cpth_len - 1] - cpth[cpth_len - 2]).normalize();

                cpth[cpth_len - 1] = cpth[cpth_len - 2]
                    + dir * (expected_distance - cumulative_length.last().unwrap()) as f32;

                cumulative_length.push(expected_distance);
            }
        }

        CalculatedLength {
            cumulative_length,
            segment_end_distances,
        }
    }
}
