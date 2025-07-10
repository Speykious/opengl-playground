#![allow(clippy::manual_memcpy)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::get_first)]

use std::f64::consts::TAU;

use glam::{Vec2, vec2};

use crate::scenes::osu_slider::slider_path::SliderControlPoint;

pub const CIRCULAR_ARC_TOLERANCE: f64 = 0.1;
pub const CATMULL_DETAIL: usize = 50;
pub const CATMULL_SEGMENT_LENGTH: usize = CATMULL_DETAIL * 2;

/// Creates a piecewise-linear approximation of a linear curve.
/// Basically, returns the input.
pub fn linear_to_piecewise_linear(control_points: &[SliderControlPoint]) -> Vec<Vec2> {
    control_points.iter().map(SliderControlPoint::vec2).collect::<Vec<_>>()
}

/// Creates a piecewise-linear approximation of a Catmull-Rom spline.
pub fn catmull_to_piecewise_linear(control_points: &[SliderControlPoint]) -> Vec<Vec2> {
    let mut result = Vec::with_capacity((control_points.len() - 1) * CATMULL_SEGMENT_LENGTH);

    for i in 0..(control_points.len() - 1) {
        let v1 = if i > 0 {
            control_points[i - 1].vec2()
        } else {
            control_points[i].vec2()
        };
        let v2 = control_points[i].vec2();
        let v3 = if i < control_points.len() - 1 {
            control_points[i + 1].vec2()
        } else {
            v2 + v2 - v1
        };
        let v4 = if i < control_points.len() - 2 {
            control_points[i + 2].vec2()
        } else {
            v3 + v3 - v2
        };

        for c in 0..CATMULL_DETAIL {
            result.push(catmull_find_point(v1, v2, v3, v4, c as f32 / CATMULL_DETAIL as f32));
            result.push(catmull_find_point(
                v1,
                v2,
                v3,
                v4,
                (c + 1) as f32 / CATMULL_DETAIL as f32,
            ));
        }
    }

    result
}

pub fn bezier_to_piecewise_linear(control_points: &[SliderControlPoint]) -> Vec<Vec2> {
    // Spline fitting does not make sense when the input contains no points or just one point.
    // In this case the user likely wants this function to behave like a no-op.
    if control_points.len() < 2 {
        return if control_points.is_empty() {
            Vec::new()
        } else {
            vec![control_points[0].vec2()]
        };
    }

    let degree = control_points.len() - 1;

    let mut output = Vec::new();
    let point_count = control_points.len() - 1;

    let mut to_flatten = vec![(control_points.iter()).map(|cp| cp.vec2()).collect::<Vec<_>>()];
    let mut free_buffers = Vec::new();

    // "to_flatten" contains all the curves which are not yet approximated well enough.
    // We use a stack to emulate recursion without the risk of running into a stack overflow.
    // (More specifically, we iteratively and adaptively refine our curve with a Depth-first
    // search over the tree resulting from the subdivisions we make.)

    let mut subdivision_buffer1 = vec![Vec2::ZERO; degree + 1];
    let mut subdivision_buffer2 = vec![Vec2::ZERO; degree * 2 + 1];

    let left_child = &mut subdivision_buffer2;

    while let Some(mut parent) = to_flatten.pop() {
        if bezier_is_flat_enough(&parent) {
            // If the control points we currently operate on are sufficiently "flat", we use
            // an extension to De Casteljau's algorithm to obtain a piecewise-linear approximation
            // of the bezier curve represented by our control points, consisting of the same amount
            // of points as there are control points.
            bezier_approximate(&parent, &mut output, &mut subdivision_buffer1, left_child, degree + 1);

            free_buffers.push(parent);
            continue;
        }

        // If we do not yet have a sufficiently "flat" (in other words, detailed) approximation we keep
        // subdividing the curve we are currently operating on.
        let mut right_child = (free_buffers.pop()).unwrap_or_else(|| vec![Vec2::ZERO; degree + 1]);
        bezier_subdivide(
            &parent,
            left_child,
            &mut right_child,
            Some(&mut subdivision_buffer1),
            degree + 1,
        );

        // We re-use the buffer of the parent for one of the children, so that we save one allocationbezierSubdivide per iteration.
        for i in 0..(degree + 1) {
            parent[i] = left_child[i];
        }

        to_flatten.push(right_child);
        to_flatten.push(parent);
    }

    output.push(control_points[point_count].vec2());
    output
}

fn bezier_is_flat_enough(control_points: &[Vec2]) -> bool {
    const BEZIER_TOLERANCE: f32 = 0.25;

    for i in 1..(control_points.len() - 1) {
        if (control_points[i - 1] - 2.0 * control_points[i] + control_points[i + 1]).length_squared()
            > BEZIER_TOLERANCE * BEZIER_TOLERANCE * 4.0
        {
            return false;
        }
    }

    true
}

fn bezier_approximate(
    control_points: &[Vec2],
    output: &mut Vec<Vec2>,
    subdivision_buffer1: &mut [Vec2],
    subdivision_buffer2: &mut [Vec2],
    count: usize,
) {
    let l = subdivision_buffer2;
    let r = subdivision_buffer1;

    bezier_subdivide(control_points, l, r, None, count);

    for i in 0..(count - 1) {
        l[count + i] = r[i + 1];
    }

    output.push(control_points[0]);

    for i in 1..(count - 1) {
        let index = 2 * i;
        let p = 0.25 * (l[index - 1] + 2.0 * l[index] + l[index + 1]);
        output.push(p);
    }
}

/// Subdivides n control points representing a bezier curve into 2 sets of n control points, each
/// describing a bezier curve equivalent to a half of the original curve. Effectively this splits
/// the original curve into 2 curves which result in the original curve when pieced back together.
///
/// If the `subdivision_buffer` is `None`, then `r` contains the current subdivision state.
fn bezier_subdivide(
    control_points: &[Vec2],
    l: &mut [Vec2],
    r: &mut [Vec2],
    subdivision_buffer: Option<&mut [Vec2]>,
    count: usize,
) {
    // The borrow checker made me butcher this function to death. Twice.
    // It was very infuriating.

    if let Some(s) = subdivision_buffer {
        s[..count].copy_from_slice(&control_points[..count]);

        for i in 0..count {
            l[i] = s[0];
            r[count - i - 1] = s[count - i - 1];

            for j in 0..(count - i - 1) {
                s[j] = (s[j] + s[j + 1]) / 2.0;
            }
        }
    } else {
        r[..count].copy_from_slice(&control_points[..count]);

        for i in 0..count {
            l[i] = r[0];
            // r[count - i - 1] = r[count - i - 1];

            for j in 0..(count - i - 1) {
                r[j] = (r[j] + r[j + 1]) / 2.0;
            }
        }
    }
}

#[rustfmt::skip]
fn catmull_find_point(v1: Vec2, v2: Vec2, v3: Vec2, v4: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t * t2;

    Vec2 {
        x: 0.5 * (2.0 * v2.x + (-v1.x + v3.x) * t + (2.0 * v1.x - 5.0 * v2.x + 4.0 * v3.x - v4.x) * t2 + (-v1.x + 3.0 * v2.x - 3.0 * v3.x + v4.x) * t3),
        y: 0.5 * (2.0 * v2.y + (-v1.y + v3.y) * t + (2.0 * v1.y - 5.0 * v2.y + 4.0 * v3.y - v4.y) * t2 + (-v1.y + 3.0 * v2.y - 3.0 * v3.y + v4.y) * t3),
    }
}

/// Creates a piecewise-linear approximation of a circular arc curve.
pub fn circular_arc_to_piecewise_linear(carps: CircularArcProps) -> Vec<Vec2> {
    // We select the amount of points for the approximation by requiring the discrete curvature
    // to be smaller than the provided tolerance. The exact angle required to meet the tolerance
    // is: 2 * Math.Acos(1 - TOLERANCE / r)
    // The special case is required for extremely short sliders where the radius is smaller than
    // the tolerance. This is a pathological rather than a realistic case.
    let amount_points = match 2.0 * carps.radius as f64 <= CIRCULAR_ARC_TOLERANCE {
        true => 2,
        false => 2.max(
            (carps.theta_range / (2.0 * (1.0 - CIRCULAR_ARC_TOLERANCE / carps.radius as f64).acos())).ceil() as usize,
        ),
    };

    (0..amount_points)
        .map(|i| {
            let fract = i as f64 / (amount_points - 1) as f64;
            let theta = carps.theta_start + carps.direction * fract * carps.theta_range;
            carps.center + vec2(theta.cos() as f32, theta.sin() as f32) * carps.radius
        })
        .collect::<Vec<_>>()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircularArcProps {
    pub theta_start: f64,
    pub theta_range: f64,
    pub direction: f64,
    pub radius: f32,
    pub center: Vec2,
}

/// Computes various properties that can be used to approximate the circular arc.
pub fn circular_arc_props(control_points: &[SliderControlPoint]) -> Option<CircularArcProps> {
    let a = control_points.get(0)?.vec2();
    let b = control_points.get(1)?.vec2();
    let c = control_points.get(2)?.vec2();

    // If we have a degenerate triangle where a side-length is almost zero, then give up and fallback to a more numerically stable method.
    if ((b.y - a.y) * (c.x - a.x) - (b.x - a.x) * (c.y - a.y)).abs() < 0.001 {
        return None;
    }

    // See: https://en.wikipedia.org/wiki/Circumscribed_circle#Cartesian_coordinates_2
    let d = 2.0 * (a.x * (b - c).y + b.x * (c - a).y + c.x * (a - b).y);
    let a_sq = a.length_squared();
    let b_sq = b.length_squared();
    let c_sq = c.length_squared();

    let center = vec2(
        a_sq * (b - c).y + b_sq * (c - a).y + c_sq * (a - b).y,
        a_sq * (c - b).x + b_sq * (a - c).x + c_sq * (b - a).x,
    ) / d;

    let da = a - center;
    let dc = c - center;

    let radius = da.length();
    let theta_start = (da.y as f64).atan2(da.x as f64);
    let theta_end = {
        let mut theta_end = (dc.y as f64).atan2(dc.x as f64);
        while theta_end < theta_start {
            theta_end += TAU;
        }
        theta_end
    };

    let mut direction = 1.0;
    let mut theta_range = theta_end - theta_start;

    // Decide in which direction to draw the circle, depending on which side of AC B lies.
    let ortho_a_to_c = c - a;
    let ortho_a_to_c = vec2(ortho_a_to_c.y, -ortho_a_to_c.x);
    if ortho_a_to_c.dot(b - a).is_sign_negative() {
        direction = -direction;
        theta_range = TAU - theta_range;
    }

    Some(CircularArcProps {
        theta_start,
        theta_range,
        direction,
        radius,
        center,
    })
}
