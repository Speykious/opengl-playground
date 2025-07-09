//! Logic taken from osu.Framework.Graphics.Lines.PathDrawNode

use std::f32::consts::{PI, TAU};

use glam::{vec2, Vec2};

use crate::scenes::osu_slider::SliderVertex;

#[derive(Debug, Clone, Copy)]
struct Line {
    pub a: Vec2,
    pub b: Vec2,
}

impl Line {
    pub const fn new(a: Vec2, b: Vec2) -> Self {
        Self { a, b }
    }

    pub fn direction(&self) -> Vec2 {
        self.b - self.a
    }

    pub fn direction_normalized(&self) -> Vec2 {
        self.direction().normalize()
    }

    pub fn orthogonal_direction(&self) -> Vec2 {
        let dir = self.direction_normalized();
        vec2(-dir.y, dir.x)
    }

    pub fn theta(&self) -> f32 {
        (self.b.y - self.a.y).atan2(self.b.x - self.a.x)
    }
}

pub fn generate_slider_vertices(path: &[Vec2], radius: f32) -> Vec<SliderVertex> {
    // Taken from osu.Framework.Graphics.Lines.PathDrawNode.updateVertexBuffer()

    // Explanation of the terms "left" and "right":
    // "Left" and "right" are used here in terms of a typical (Cartesian) coordinate system.
    // So "left" corresponds to positive angles (anti-clockwise), and "right" corresponds
    // to negative angles (clockwise).
    //
    // Note that this is not the same as the actually used coordinate system, in which the
    // y-axis is flipped. In this system, "left" corresponds to negative angles (clockwise)
    // and "right" corresponds to positive angles (anti-clockwise).
    //
    // Using a Cartesian system makes the calculations more consistent with typical math,
    // such as in angle<->coordinate conversions and ortho vectors. For example, the x-unit
    // vector (1, 0) has the orthogonal y-unit vector (0, 1). This would be "left" in the
    // Cartesian system. But in the actual system, it's "right" and clockwise. Where
    // this becomes confusing is during debugging, because OpenGL uses a Cartesian system.
    // So to make debugging a bit easier (i.e. w/ RenderDoc or Nsight), this code uses terms
    // that make sense in the realm of OpenGL, rather than terms which  are technically
    // accurate in the actually used "flipped" system.

    let mut vertices = Vec::new();

    let mut prev_seg: Option<Line> = None;
    let mut prev_seg_l: Option<Line> = None;
    let mut prev_seg_r: Option<Line> = None;

    let segments = (path.windows(2))
        .map(|slice| match slice {
            &[a, b] => Line::new(a, b),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();

    for (i, &seg) in segments.iter().enumerate() {
        let mut ortho = seg.orthogonal_direction();
        if ortho.x.is_nan() || ortho.y.is_nan() {
            ortho = Vec2::Y;
        }

        let curr_seg_l = Line::new(seg.a + ortho * radius, seg.b + ortho * radius);
        let curr_seg_r = Line::new(seg.a - ortho * radius, seg.b - ortho * radius);

        add_segment_quads(&mut vertices, seg, curr_seg_l, curr_seg_r);

        if let (Some(prev_seg), Some(prev_seg_l), Some(prev_seg_r)) =
            (prev_seg, prev_seg_l, prev_seg_r)
        {
            // Connection/filler caps between segment quads
            let theta_diff = seg.theta() - prev_seg.theta();
            add_segment_caps(
                &mut vertices,
                radius,
                theta_diff,
                curr_seg_l,
                curr_seg_r,
                prev_seg_l,
                prev_seg_r,
            );
        }

        // Explanation of semi-circle caps:
        // Semi-circles are essentially 180 degree caps. So to create these caps, we
        // can simply "fake" a segment that's 180 degrees flipped. This works because
        // we are taking advantage of the fact that a path which makes a 180 degree
        // bend would have a semi-circle cap.

        if i == 0 {
            // Path start cap (semi-circle);
            let flipped_l = Line::new(curr_seg_r.b, curr_seg_r.a);
            let flipped_r = Line::new(curr_seg_l.b, curr_seg_l.a);
            add_segment_caps(
                &mut vertices,
                radius,
                PI,
                curr_seg_l,
                curr_seg_r,
                flipped_l,
                flipped_r,
            );
        }

		if i == segments.len() - 1 {
			// Path end cap (semi-circle)
			let flipped_l = Line::new(curr_seg_r.b, curr_seg_r.a);
            let flipped_r = Line::new(curr_seg_l.b, curr_seg_l.a);
            add_segment_caps(
                &mut vertices,
                radius,
                PI,
                flipped_l,
                flipped_r,
                curr_seg_l,
                curr_seg_r,
            );
		}

        prev_seg = Some(seg);
        prev_seg_l = Some(curr_seg_l);
        prev_seg_r = Some(curr_seg_r);
    }

    vertices
}

fn add_segment_quads(vertices: &mut Vec<SliderVertex>, seg: Line, seg_l: Line, seg_r: Line) {
    // Each segment of the path is actually rendered as 2 quads, being split in half along the approximating line.
    // On this line the depth is 1 instead of 0, which is done in order to properly handle self-overlap using the depth buffer.
    let first_middle_point = seg.a.extend(-1.0);
    let second_middle_point = seg.b.extend(-1.0);

    // Each of the quads (mentioned above) is rendered as 2 triangles:

    // Outer quad, triangle 1
    vertices.push(SliderVertex::new(seg_r.b.extend(0.0), Vec2::ONE));
    vertices.push(SliderVertex::new(seg_r.a.extend(0.0), Vec2::ONE));
    vertices.push(SliderVertex::new(first_middle_point, Vec2::ZERO));

    // Outer quad, triangle 2
    vertices.push(SliderVertex::new(first_middle_point, Vec2::ZERO));
    vertices.push(SliderVertex::new(second_middle_point, Vec2::ZERO));
    vertices.push(SliderVertex::new(seg_r.b.extend(0.0), Vec2::ONE));

    // Inner quad, triangle 1
    vertices.push(SliderVertex::new(first_middle_point, Vec2::ZERO));
    vertices.push(SliderVertex::new(second_middle_point, Vec2::ZERO));
    vertices.push(SliderVertex::new(seg_l.b.extend(0.0), Vec2::ONE));

    // Inner quad, triangle 2
    vertices.push(SliderVertex::new(seg_l.b.extend(0.0), Vec2::ONE));
    vertices.push(SliderVertex::new(seg_l.a.extend(0.0), Vec2::ONE));
    vertices.push(SliderVertex::new(first_middle_point, Vec2::ZERO));
}

fn add_segment_caps(
    vertices: &mut Vec<SliderVertex>,
    radius: f32,
    theta_diff: f32,
    seg_l: Line,
    seg_r: Line,
    prev_seg_l: Line,
    prev_seg_r: Line,
) {
    const MAX_RESOLUTION: u32 = 24;

    let theta_diff = if theta_diff.abs() > PI {
        theta_diff - theta_diff.signum() * TAU
    } else {
        theta_diff
    };

    if theta_diff == 0.0 {
        return;
    }

    let theta_diff_is_pos = theta_diff.is_sign_positive();

    let origin = seg_l.a + (seg_r.a - seg_l.a) * 0.5;

    // Use segment end points instead of calculating start/end via theta to guarantee
    // that the vertices have the exact same position as the quads, which prevents
    // possible pixel gaps during rasterization.
    let mut current = if theta_diff_is_pos {
        prev_seg_r.b
    } else {
        prev_seg_l.b
    };

    let end = if theta_diff_is_pos { seg_r.a } else { seg_l.a };

    let start = if theta_diff_is_pos {
        Line::new(prev_seg_l.b, prev_seg_r.b)
    } else {
        Line::new(prev_seg_r.b, prev_seg_l.b)
    };

    let theta0 = start.theta();
    let theta_step = theta_diff.signum() * PI / MAX_RESOLUTION as f32;
    let step_count = (theta_diff / theta_step).ceil() as usize;

    for i in 1..=step_count {
        // Center point
        vertices.push(SliderVertex::new(origin.extend(-1.0), Vec2::ZERO));

        // First outer point
        vertices.push(SliderVertex::new(current.extend(0.0), Vec2::ONE));

        current = if i < step_count {
            origin + Vec2::from_angle(theta0 + i as f32 * theta_step) * radius
        } else {
            end
        };

        // Second outer point
        vertices.push(SliderVertex::new(current.extend(0.0), Vec2::ONE));
    }
}
