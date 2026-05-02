use crate::cube::Move;
use macroquad::prelude::Vec3;

/// Snapshot of which world axes correspond to the camera's view directions.
/// All three axes (`front`, `right`, `up`) are guaranteed to be distinct.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ViewBasis {
    pub front_axis: u8,
    pub front_sign: i8,
    pub right_axis: u8,
    pub right_sign: i8,
    pub up_axis: u8,
    pub up_sign: i8,
}

impl ViewBasis {
    #[allow(dead_code)]
    pub fn from_camera(azimuth: f32, elevation: f32) -> Self {
        let (front, right, up) = view_axes(azimuth, elevation);
        Self {
            front_axis: front.0,
            front_sign: front.1,
            right_axis: right.0,
            right_sign: right.1,
            up_axis: up.0,
            up_sign: up.1,
        }
    }

    /// Build from explicit camera basis vectors in world coords. Used when
    /// the camera is parameterized by quaternion rather than azimuth/elevation.
    /// `pos_dir` is the unit vector from target to camera.
    pub fn from_vectors(pos_dir: Vec3, right: Vec3, up: Vec3) -> Self {
        let (f, r, u) = view_axes_from_vectors(pos_dir, right, up);
        Self {
            front_axis: f.0,
            front_sign: f.1,
            right_axis: r.0,
            right_sign: r.1,
            up_axis: u.0,
            up_sign: u.1,
        }
    }

    #[cfg(test)]
    pub fn front_label(&self) -> &'static str {
        face_label(self.front_axis, self.front_sign)
    }

    #[cfg(test)]
    pub fn right_label(&self) -> &'static str {
        face_label(self.right_axis, self.right_sign)
    }
}

#[cfg(test)]
pub fn face_label(axis: u8, sign: i8) -> &'static str {
    match (axis, sign) {
        (0, 1) => "R",
        (0, -1) => "L",
        (1, 1) => "U",
        (1, -1) => "D",
        (2, 1) => "F",
        (2, -1) => "B",
        _ => "?",
    }
}

/// Full 3D front-face detection: which of the six cube faces is currently
/// most aligned with the camera's position direction.
#[allow(dead_code)]
pub fn front_3d(azimuth: f32, elevation: f32) -> (u8, i8) {
    let cx = elevation.cos() * azimuth.cos();
    let cy = elevation.sin();
    let cz = elevation.cos() * azimuth.sin();
    let ax = cx.abs();
    let ay = cy.abs();
    let az = cz.abs();
    if ay >= ax && ay >= az {
        (1, if cy >= 0.0 { 1 } else { -1 })
    } else if ax >= az {
        (0, if cx >= 0.0 { 1 } else { -1 })
    } else {
        (2, if cz >= 0.0 { 1 } else { -1 })
    }
}

/// Same as [`view_axes`], but takes the camera basis vectors directly.
/// Use this with the quaternion-based [`OrbitCamera`] which doesn't expose
/// azimuth/elevation. `pos_dir` is the unit vector from target to camera.
///
/// Picks the dominant world axis for each of front / right / up. The three
/// axes must end up distinct; if they collide (which can happen at exact
/// 45° boundary orientations), right and up are reassigned to the two axes
/// perpendicular to front based on the larger projection.
pub fn view_axes_from_vectors(
    pos_dir: Vec3,
    right: Vec3,
    up: Vec3,
) -> ((u8, i8), (u8, i8), (u8, i8)) {
    let (front_axis, front_sign) = dominant_axis(pos_dir);
    let (right_axis, right_sign) = dominant_axis(right);
    let (up_axis, up_sign) = dominant_axis(up);

    if front_axis != right_axis && front_axis != up_axis && right_axis != up_axis {
        return (
            (front_axis, front_sign),
            (right_axis, right_sign),
            (up_axis, up_sign),
        );
    }

    // Collision recovery: right and up share an axis (or one of them collides
    // with front). Force them onto the two world axes perpendicular to front,
    // assigning right to whichever axis it projects onto more strongly.
    let (a, b) = match front_axis {
        0 => (1u8, 2u8),
        1 => (0u8, 2u8),
        2 => (0u8, 1u8),
        _ => unreachable!(),
    };
    let pr_a = component_at(right, a);
    let pr_b = component_at(right, b);
    let (r_axis, r_sign) = if pr_a.abs() >= pr_b.abs() {
        (a, sign_of(pr_a))
    } else {
        (b, sign_of(pr_b))
    };
    let u_axis = if r_axis == a { b } else { a };
    let u_sign = sign_of(component_at(up, u_axis));
    ((front_axis, front_sign), (r_axis, r_sign), (u_axis, u_sign))
}

fn component_at(v: Vec3, axis: u8) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn dominant_axis(v: Vec3) -> (u8, i8) {
    let ax = v.x.abs();
    let ay = v.y.abs();
    let az = v.z.abs();
    if ay >= ax && ay >= az {
        (1, sign_of(v.y))
    } else if ax >= az {
        (0, sign_of(v.x))
    } else {
        (2, sign_of(v.z))
    }
}

fn sign_of(x: f32) -> i8 {
    if x >= 0.0 {
        1
    } else {
        -1
    }
}

/// Azimuth/elevation variant kept for the unit tests. Runtime callers
/// use `view_axes_from_vectors` with the quaternion-based camera.
#[allow(dead_code)]
pub fn view_axes(azimuth: f32, elevation: f32) -> ((u8, i8), (u8, i8), (u8, i8)) {
    let cx = elevation.cos() * azimuth.cos();
    let cy = elevation.sin();
    let cz = elevation.cos() * azimuth.sin();

    let (front_axis, front_sign) = front_3d(azimuth, elevation);

    // Camera screen-right and screen-up vectors in world coords.
    // forward = -cam, side = forward × world_up, up_view = side × forward.
    let fx = -cx;
    let fy = -cy;
    let fz = -cz;
    let mut sx = -fz;
    let mut sy = 0.0;
    let mut sz = fx;
    let smag = (sx * sx + sy * sy + sz * sz).sqrt();
    if smag < 1e-4 {
        sx = azimuth.sin();
        sy = 0.0;
        sz = -azimuth.cos();
    } else {
        sx /= smag;
        sy /= smag;
        sz /= smag;
    }
    let ux = sy * fz - sz * fy;
    let uy = sz * fx - sx * fz;
    let uz = sx * fy - sy * fx;
    let umag = (ux * ux + uy * uy + uz * uz).sqrt().max(1e-6);
    let ux = ux / umag;
    let uz = uz / umag;
    let _ = uy;

    if front_axis == 1 {
        let (right_axis, right_sign) = if sx.abs() >= sz.abs() {
            (0u8, if sx >= 0.0 { 1i8 } else { -1 })
        } else {
            (2, if sz >= 0.0 { 1 } else { -1 })
        };
        let (up_axis, up_sign) = if right_axis == 0 {
            (2u8, if uz >= 0.0 { 1i8 } else { -1 })
        } else {
            (0u8, if ux >= 0.0 { 1i8 } else { -1 })
        };
        (
            (front_axis, front_sign),
            (right_axis, right_sign),
            (up_axis, up_sign),
        )
    } else {
        let (right_axis, right_sign) = if sx.abs() >= sz.abs() {
            (0u8, if sx >= 0.0 { 1i8 } else { -1 })
        } else {
            (2, if sz >= 0.0 { 1 } else { -1 })
        };
        (
            (front_axis, front_sign),
            (right_axis, right_sign),
            (1u8, 1i8),
        )
    }
}

/// Returns ±1 indicating the signed quarter-turns around +`axis` that
/// rotate the world unit vector `from` to `to`. Both `from` and `to` must be
/// axis-aligned unit vectors perpendicular to `axis`, and they must differ.
fn quarter_turns_to(axis: u8, from: (u8, i8), to: (u8, i8)) -> i8 {
    debug_assert_ne!(from.0, axis);
    debug_assert_ne!(to.0, axis);
    debug_assert!(from != to);

    // Cyclic order around +axis (right-hand rule).
    //   +X axis: +Y → +Z → -Y → -Z
    //   +Y axis: +Z → +X → -Z → -X
    //   +Z axis: +X → +Y → -X → -Y
    let next_axis = match axis {
        0 => 1u8,
        1 => 2,
        2 => 0,
        _ => unreachable!(),
    };

    let pos = |v: (u8, i8)| -> u8 {
        if v.0 == next_axis {
            if v.1 > 0 {
                0
            } else {
                2
            }
        } else {
            if v.1 > 0 {
                1
            } else {
                3
            }
        }
    };

    let diff = (pos(to) + 4 - pos(from)) % 4;
    match diff {
        1 => 1,
        3 => -1,
        2 => 2,
        _ => 0,
    }
}

/// Row rotation (numpad keys 7/4/1/9/6/3) — view-relative.
/// `key_top`: +1 = top row, 0 = mid, -1 = bot row.
/// `right`: true = visual front edge moves to the visual right (keys 9/6/3),
///          false = moves to the visual left (keys 7/4/1).
pub fn row_move(view: ViewBasis, key_top: i8, right: bool) -> Move {
    let layer = key_top * view.up_sign;
    let dest_sign = if right {
        view.right_sign
    } else {
        -view.right_sign
    };
    let turns = quarter_turns_to(
        view.up_axis,
        (view.front_axis, view.front_sign),
        (view.right_axis, dest_sign),
    );
    Move::slice(view.up_axis, layer, turns)
}

/// Column rotation (Shift+1/2/3/7/8/9) — view-relative.
/// `col_idx`: -1 = visual left col, 0 = mid, +1 = visual right col.
/// `fwd`: true = top of column rolls toward viewer; false = away.
pub fn col_move(view: ViewBasis, col_idx: i8, fwd: bool) -> Move {
    let layer = col_idx * view.right_sign;
    let dest_sign = if fwd {
        view.front_sign
    } else {
        -view.front_sign
    };
    let turns = quarter_turns_to(
        view.right_axis,
        (view.up_axis, view.up_sign),
        (view.front_axis, dest_sign),
    );
    Move::slice(view.right_axis, layer, turns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn vb(az: f32) -> ViewBasis {
        ViewBasis::from_camera(az, 0.0)
    }

    fn vb3(az: f32, el: f32) -> ViewBasis {
        ViewBasis::from_camera(az, el)
    }

    #[test]
    fn front_face_at_cardinals() {
        assert_eq!(vb(0.0).front_label(), "R");
        assert_eq!(vb(PI / 2.0).front_label(), "F");
        assert_eq!(vb(PI).front_label(), "L");
        assert_eq!(vb(-PI / 2.0).front_label(), "B");
        assert_eq!(vb(3.0 * PI / 2.0).front_label(), "B");
    }

    #[test]
    fn right_face_at_cardinals() {
        assert_eq!(vb(PI / 2.0).right_label(), "R");
        assert_eq!(vb(0.0).right_label(), "B");
        assert_eq!(vb(-PI / 2.0).right_label(), "L");
        assert_eq!(vb(PI).right_label(), "F");
    }

    // --- Row rotations (key 7) — F/R/B/L horizontal views ---

    #[test]
    fn row_top_left_horizontal_views_rotate_y_top() {
        // For all horizontal views, key 7 should rotate y=+1 layer with -1 turn.
        // (Same Y-axis behavior as before this refactor.)
        for &az in &[PI / 2.0, 0.0, -PI / 2.0, PI] {
            let m = row_move(vb(az), 1, false);
            assert_eq!(m.axis, 1, "az={}", az);
            assert_eq!(m.layer, 1, "az={}", az);
            assert_eq!(m.turns, -1, "az={}", az);
        }
    }

    // --- Row rotations on U-view ---

    #[test]
    fn row_top_left_u_view_rotates_per_screen_up() {
        // U-view at azimuth π/2: screen up = -Z. Top row = z=-1 layer.
        // Pressing 7 should rotate around +Z by +1 turn (so +Y → -X = visual left).
        let v = vb3(PI / 2.0, PI / 2.0 - 0.05);
        let m = row_move(v, 1, false);
        assert_eq!(m.axis, 2);
        assert_eq!(m.layer, -1);
        assert_eq!(m.turns, 1);
    }

    // --- Row rotations on D-view ---

    #[test]
    fn row_top_left_d_view_at_az0_rotates_around_x() {
        // D-view at az=0: front=D, right=B (Z=-1), up=R (X=+1).
        // Top row = x=+1 layer. Front=-Y → left=+Z around +X needs -1 turn.
        let v = vb3(0.0, -PI / 2.0 + 0.05);
        let m = row_move(v, 1, false);
        assert_eq!(m.axis, 0);
        assert_eq!(m.layer, 1);
        assert_eq!(m.turns, -1);
    }

    // --- Column rotations preserved ---

    #[test]
    fn shift1_front_f_targets_l_layer() {
        let v = vb(PI / 2.0);
        let m = col_move(v, -1, true);
        assert_eq!(m.axis, 0);
        assert_eq!(m.layer, -1);
    }

    #[test]
    fn shift1_front_r_targets_f_layer() {
        let v = vb(0.0);
        let m = col_move(v, -1, true);
        assert_eq!(m.axis, 2);
        assert_eq!(m.layer, 1);
    }

    #[test]
    fn shift1_front_b_targets_r_layer() {
        let v = vb(-PI / 2.0);
        let m = col_move(v, -1, true);
        assert_eq!(m.axis, 0);
        assert_eq!(m.layer, 1);
    }

    #[test]
    fn shift1_front_l_targets_b_layer() {
        let v = vb(PI);
        let m = col_move(v, -1, true);
        assert_eq!(m.axis, 2);
        assert_eq!(m.layer, -1);
    }

    #[test]
    fn shift1_layer_changes_with_view() {
        let layers: Vec<(u8, i8)> = [PI / 2.0, 0.0, -PI / 2.0, PI]
            .iter()
            .map(|&az| {
                let m = col_move(vb(az), -1, true);
                (m.axis, m.layer)
            })
            .collect();
        assert_eq!(layers[0], (0, -1));
        assert_eq!(layers[1], (2, 1));
        assert_eq!(layers[2], (0, 1));
        assert_eq!(layers[3], (2, -1));
    }

    #[test]
    fn shift_fwd_then_bwd_cancels() {
        for az in [0.0, PI / 2.0, PI, -PI / 2.0] {
            let v = vb(az);
            for col in [-1i8, 0, 1] {
                let f = col_move(v, col, true);
                let b = col_move(v, col, false);
                assert_eq!(f.axis, b.axis);
                assert_eq!(f.layer, b.layer);
                assert_eq!(f.turns, -b.turns);
            }
        }
    }

    #[test]
    fn row_left_then_right_cancels_all_views() {
        // Including U/D views, pressing left then right on the same row
        // should produce inverse moves.
        let cases = [
            (PI / 2.0, 0.0),
            (0.0, 0.0),
            (-PI / 2.0, 0.0),
            (PI, 0.0),
            (PI / 2.0, PI / 2.0 - 0.05),
            (0.0, -PI / 2.0 + 0.05),
        ];
        for (az, el) in cases {
            let v = vb3(az, el);
            for row in [-1i8, 0, 1] {
                let l = row_move(v, row, false);
                let r = row_move(v, row, true);
                assert_eq!(l.axis, r.axis);
                assert_eq!(l.layer, r.layer);
                assert_eq!(l.turns, -r.turns);
            }
        }
    }

    #[test]
    fn front_3d_detects_top_bottom() {
        assert_eq!(front_3d(0.0, PI / 2.0 - 0.01), (1, 1));
        assert_eq!(front_3d(PI / 2.0, PI / 2.0 - 0.01), (1, 1));
        assert_eq!(front_3d(0.0, -PI / 2.0 + 0.01), (1, -1));
        assert_eq!(front_3d(PI, -PI / 2.0 + 0.01), (1, -1));
    }

    #[test]
    fn view_axes_horizontal_views() {
        let (f, r, u) = view_axes(PI / 2.0, 0.0);
        assert_eq!(f, (2, 1));
        assert_eq!(r, (0, 1));
        assert_eq!(u, (1, 1));
        let (f, r, u) = view_axes(0.0, 0.0);
        assert_eq!(f, (0, 1));
        assert_eq!(r, (2, -1));
        assert_eq!(u, (1, 1));
    }

    #[test]
    fn view_axes_top_view_three_distinct_axes() {
        for &az in &[0.0_f32, PI / 2.0, PI, -PI / 2.0, 0.7, 1.3] {
            let (f, r, u) = view_axes(az, PI / 2.0 - 0.05);
            assert_eq!(f.0, 1);
            assert_eq!(f.1, 1);
            assert_ne!(r.0, u.0, "right and up must differ at az={}", az);
            assert_ne!(r.0, f.0);
            assert_ne!(u.0, f.0);
        }
    }

    #[test]
    fn view_axes_bottom_view_three_distinct_axes() {
        for &az in &[0.0_f32, PI / 2.0, PI, -PI / 2.0, 0.7, 1.3] {
            let (f, r, u) = view_axes(az, -PI / 2.0 + 0.05);
            assert_eq!(f, (1, -1));
            assert_ne!(r.0, u.0);
        }
    }

    #[test]
    fn front_3d_falls_back_to_horizontal_at_low_elevation() {
        assert_eq!(front_3d(PI / 2.0, 0.0), (2, 1));
        assert_eq!(front_3d(0.0, 0.0), (0, 1));
        assert_eq!(front_3d(PI, 0.0), (0, -1));
        assert_eq!(front_3d(-PI / 2.0, 0.0), (2, -1));
    }
}
