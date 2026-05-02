use crate::animation::Animator;
use crate::cube::Cube;
use macroquad::camera::Camera as _;
use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

/// Project a world-space point into screen pixel coordinates using the
/// given 3D camera. Equivalent to Camera2D::world_to_screen, which macroquad
/// only provides for 2D cameras.
pub(crate) fn world_to_screen_3d(camera: &Camera3D, point: Vec3) -> Vec2 {
    let ndc = camera.matrix().project_point3(point);
    Vec2::new(
        (ndc.x * 0.5 + 0.5) * screen_width(),
        (-ndc.y * 0.5 + 0.5) * screen_height(),
    )
}

const SPACING: f32 = 1.0;
const BODY_SIZE: f32 = 0.96;
const BODY_HALF: f32 = BODY_SIZE * 0.5;
const STICKER_HALF: f32 = 0.42;
const STICKER_OFFSET: f32 = BODY_HALF + 0.005;

const COLOR_U: Color = WHITE;
const COLOR_D: Color = YELLOW;
const COLOR_F: Color = GREEN;
const COLOR_B: Color = BLUE;
const COLOR_R: Color = RED;
const COLOR_L: Color = ORANGE;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PinAnchor {
    pub home: IVec3,
    pub dir: IVec3,
}

/// Quaternion-based orbit camera. No gimbal lock — the camera can be rotated
/// freely in any direction. The current and target orientations are kept
/// distinct so animations slerp smoothly from one to the other.
pub struct OrbitCamera {
    pub orientation: Quat,
    pub target_orientation: Quat,
    pub distance: f32,
    pub target: Vec3,
}

const ANGULAR_SPEED: f32 = 8.0; // rad/sec — slerp rate toward target

impl OrbitCamera {
    pub fn new() -> Self {
        // Default view: camera sits in the +X +Y +Z corner, looking at origin,
        // showing F dominantly with U on top and R partially visible on the right.
        // Built from a yaw around world Y followed by a local pitch on X.
        let q_yaw = Quat::from_axis_angle(Vec3::Y, 0.5);
        let q_pitch = Quat::from_axis_angle(Vec3::X, -0.4);
        let orientation = q_yaw * q_pitch;
        Self {
            orientation,
            target_orientation: orientation,
            distance: 8.0,
            target: Vec3::ZERO,
        }
    }

    pub fn camera(&self) -> Camera3D {
        let pos = self.target + self.orientation * (Vec3::Z * self.distance);
        let up = self.orientation * Vec3::Y;
        Camera3D {
            position: pos,
            target: self.target,
            up,
            fovy: 45f32.to_radians(),
            ..Default::default()
        }
    }

    /// Camera position direction (from target out to camera), in world coords.
    pub fn pos_dir(&self) -> Vec3 {
        self.orientation * Vec3::Z
    }
    /// Camera right vector in world coords.
    pub fn right_world(&self) -> Vec3 {
        self.orientation * Vec3::X
    }
    /// Camera up vector in world coords.
    pub fn up_world(&self) -> Vec3 {
        self.orientation * Vec3::Y
    }

    /// Mouse-drag orbit: directly manipulates orientation; resyncs target.
    /// `dx` rotates around world +Y (yaw), `dy` rotates around the camera's
    /// current right axis (pitch).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let q_yaw = Quat::from_axis_angle(Vec3::Y, -dx);
        let q_pitch_local = Quat::from_axis_angle(Vec3::X, -dy);
        self.orientation = (q_yaw * self.orientation * q_pitch_local).normalize();
        self.target_orientation = self.orientation;
    }

    /// Yaw target by `angle` (positive = camera orbits CCW around +Y as seen
    /// from above, which brings the cube's right side into view).
    pub fn nudge_yaw(&mut self, angle: f32) {
        let q = Quat::from_axis_angle(Vec3::Y, -angle);
        self.target_orientation = (q * self.target_orientation).normalize();
    }

    /// Pitch target by `angle` (positive = camera tilts up over the top).
    pub fn nudge_pitch(&mut self, angle: f32) {
        let q_local = Quat::from_axis_angle(Vec3::X, -angle);
        self.target_orientation = (self.target_orientation * q_local).normalize();
    }

    /// Slerp `orientation` toward `target_orientation` by one frame.
    pub fn tick(&mut self, dt: f32) {
        let cos_half = self.orientation.dot(self.target_orientation).abs().min(1.0);
        let angle = 2.0 * cos_half.acos();
        if angle < 1e-4 {
            self.orientation = self.target_orientation;
            return;
        }
        let max_step = ANGULAR_SPEED * dt;
        let t = (max_step / angle).clamp(0.0, 1.0);
        self.orientation = self
            .orientation
            .slerp(self.target_orientation, t)
            .normalize();
    }
}

/// A blink animation targeting one face (axis + sign).
/// `intensity()` returns a brightness multiplier in [0.0, 1.0] that the
/// renderer applies to stickers of the matching face while the blink runs.
#[derive(Copy, Clone, Debug)]
pub struct Blink {
    pub axis: u8,
    pub sign: i8,
    pub elapsed: f32,
}

impl Blink {
    pub const DURATION: f32 = 1.2;
    const FREQUENCY: f32 = 4.0; // flashes per second

    pub fn new(axis: u8, sign: i8) -> Self {
        Self {
            axis,
            sign,
            elapsed: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
    }

    pub fn is_active(&self) -> bool {
        self.elapsed < Self::DURATION
    }

    /// Brightness multiplier in [0.2, 1.0] that pulses while the blink is active.
    fn intensity(&self) -> f32 {
        let phase = (self.elapsed * Self::FREQUENCY * std::f32::consts::TAU).sin();
        0.6 + 0.4 * phase // oscillates [0.2, 1.0]
    }

    /// World-space outward direction of the face being blinked.
    fn matches(&self, world_n: Vec3) -> bool {
        let axis = self.axis as usize;
        let proj = world_n[axis] * self.sign as f32;
        // Sticker is on the blinking face if its world normal points
        // strongly along the blink axis with the matching sign.
        proj > 0.7
    }
}

pub fn draw_cube_state(cube: &Cube, animator: &Animator, blink: Option<Blink>) {
    let frame = animator.current_frame();
    let (anim_q, anim_axis_layer) = match frame {
        Some((mv, t)) => (mv.quat(t), Some((mv.axis as usize, mv.layer as i32))),
        None => (Quat::IDENTITY, None),
    };

    let blink_intensity = blink.filter(|b| b.is_active()).map(|b| (b, b.intensity()));

    for cubie in &cube.cubies {
        let affected = match anim_axis_layer {
            Some((axis, layer)) => cubie.pos[axis] == layer,
            None => false,
        };
        let q_partial = if affected { anim_q } else { Quat::IDENTITY };

        let base_pos = cubie.pos.as_vec3() * SPACING;
        let world_pos = q_partial * base_pos;
        let effective_orient = q_partial * cubie.orient;

        // Render the cubie as 6 black face panels plus colored stickers on
        // the exterior faces. Drawing the body via face-quads (instead of an
        // axis-aligned `draw_cube`) keeps the body geometry locked to the
        // same rotation as the stickers, so mid-rotation frames stay clean.
        for &dir in &BODY_DIRS {
            let world_n = effective_orient * dir.as_vec3();
            let (local_u, local_v) = face_axes(dir);
            let u = effective_orient * local_u;
            let v = effective_orient * local_v;

            let body_face_center = world_pos + world_n * BODY_HALF;
            draw_quad_3d(body_face_center, u * BODY_HALF, v * BODY_HALF, BLACK);

            if let Some(mut color) = sticker_color(cubie.home, dir) {
                if let Some((b, k)) = blink_intensity {
                    if b.matches(world_n) {
                        color.r *= k;
                        color.g *= k;
                        color.b *= k;
                    }
                }
                let sticker_center = world_pos + world_n * STICKER_OFFSET;
                draw_quad_3d(sticker_center, u * STICKER_HALF, v * STICKER_HALF, color);
            }
        }
    }
}

pub fn draw_pins(cube: &Cube, animator: &Animator, pins: &[PinAnchor]) {
    let frame = animator.current_frame();
    let (anim_q, anim_axis_layer) = match frame {
        Some((mv, t)) => (mv.quat(t), Some((mv.axis as usize, mv.layer as i32))),
        None => (Quat::IDENTITY, None),
    };

    for pin in pins {
        let Some(cubie) = cube.cubies.iter().find(|c| c.home == pin.home) else {
            continue;
        };
        let affected = match anim_axis_layer {
            Some((axis, layer)) => cubie.pos[axis] == layer,
            None => false,
        };
        let q_partial = if affected { anim_q } else { Quat::IDENTITY };

        let world_pos = q_partial * (cubie.pos.as_vec3() * SPACING);
        let effective_orient = q_partial * cubie.orient;
        let world_n = (effective_orient * pin.dir.as_vec3()).normalize();
        let base = world_pos + world_n * (STICKER_OFFSET + 0.03);
        let head = base + world_n * 0.42;

        draw_line_3d(base, head, BLACK);
        draw_line_3d(base + Vec3::X * 0.015, head + Vec3::X * 0.015, BLACK);
        draw_sphere(head, 0.11, None, Color::new(1.0, 0.86, 0.12, 1.0));
        draw_sphere(base, 0.055, None, Color::new(0.1, 0.1, 0.1, 1.0));
    }
}

pub fn sticker_screen_center(camera: &Camera3D, pos: IVec3, normal: IVec3) -> Vec2 {
    let world = pos.as_vec3() * SPACING + normal.as_vec3() * (STICKER_OFFSET + 0.03);
    world_to_screen_3d(camera, world)
}

const BODY_DIRS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

fn sticker_color(home: IVec3, dir: IVec3) -> Option<Color> {
    if dir.x != 0 && home.x == dir.x {
        return Some(if dir.x > 0 { COLOR_R } else { COLOR_L });
    }
    if dir.y != 0 && home.y == dir.y {
        return Some(if dir.y > 0 { COLOR_U } else { COLOR_D });
    }
    if dir.z != 0 && home.z == dir.z {
        return Some(if dir.z > 0 { COLOR_F } else { COLOR_B });
    }
    None
}

/// In-plane (u, v) basis vectors for each cubie body face, expressed in the
/// cubie's BODY frame. They are chosen so that `u × v` points outward along
/// the face normal — i.e., the quad's two CCW triangles are front-facing
/// when viewed from outside the cubie. The renderer rotates these by
/// `effective_orient` to get the corresponding world-space u/v.
fn face_axes(dir: IVec3) -> (Vec3, Vec3) {
    match (dir.x, dir.y, dir.z) {
        (1, 0, 0) => (Vec3::Y, Vec3::Z),  // +X: Y × Z = +X
        (-1, 0, 0) => (Vec3::Z, Vec3::Y), // -X: Z × Y = -X
        (0, 1, 0) => (Vec3::Z, Vec3::X),  // +Y: Z × X = +Y
        (0, -1, 0) => (Vec3::X, Vec3::Z), // -Y: X × Z = -Y
        (0, 0, 1) => (Vec3::X, Vec3::Y),  // +Z: X × Y = +Z
        (0, 0, -1) => (Vec3::Y, Vec3::X), // -Z: Y × X = -Z
        _ => unreachable!("body face direction must be axis-aligned"),
    }
}

/// Draw the numpad-key labels (7/4/1 on the visual left column,
/// 9/6/3 on the visual right column) over the 6 stickers of the current
/// front face. The label positions are computed from the camera-relative
/// (front, right, up) axes, so they place themselves correctly on F/R/B/L
/// horizontal views as well as on U / D top-down views.
pub fn draw_front_face_labels(
    camera: &Camera3D,
    front_axis: u8,
    front_sign: i8,
    right_axis: u8,
    right_sign: i8,
    up_axis: u8,
    up_sign: i8,
    color: Color,
) {
    let labels: [(&str, i32, i32); 6] = [
        ("7", -1, 1),
        ("4", -1, 0),
        ("1", -1, -1),
        ("9", 1, 1),
        ("6", 1, 0),
        ("3", 1, -1),
    ];
    let fa = front_axis as usize;
    let ra = right_axis as usize;
    let ua = up_axis as usize;
    let rs = right_sign as i32;
    let us = up_sign as i32;
    // Place the text plane slightly in front of the sticker plane so the
    // numbers are not z-fought into the sticker geometry.
    let plane = front_sign as f32 * (SPACING + STICKER_OFFSET + 0.02);
    let font_size: f32 = 36.0;
    for &(text, col, row) in &labels {
        let mut p = [0.0f32; 3];
        p[fa] = plane;
        p[ra] = (col * rs) as f32 * SPACING;
        p[ua] = (row * us) as f32 * SPACING;
        let world = vec3(p[0], p[1], p[2]);
        let s = world_to_screen_3d(camera, world);
        let dim = measure_text(text, None, font_size as u16, 1.0);
        draw_text(
            text,
            s.x - dim.width * 0.5,
            s.y + dim.height * 0.5,
            font_size,
            color,
        );
    }
}

fn draw_quad_3d(center: Vec3, u: Vec3, v: Vec3, color: Color) {
    let p0 = center - u - v;
    let p1 = center + u - v;
    let p2 = center + u + v;
    let p3 = center - u + v;
    let mesh = Mesh {
        vertices: vec![
            Vertex::new(p0.x, p0.y, p0.z, 0.0, 0.0, color),
            Vertex::new(p1.x, p1.y, p1.z, 1.0, 0.0, color),
            Vertex::new(p2.x, p2.y, p2.z, 1.0, 1.0, color),
            Vertex::new(p3.x, p3.y, p3.z, 0.0, 1.0, color),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    };
    draw_mesh(&mesh);
}
