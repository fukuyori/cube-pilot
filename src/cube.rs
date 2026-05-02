use macroquad::prelude::{IVec3, Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    U,
    D,
    L,
    R,
    F,
    B,
}

pub const ALL_FACES: [Face; 6] = [Face::U, Face::D, Face::L, Face::R, Face::F, Face::B];

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Turn {
    Cw,
    Ccw,
    Half,
}

pub const ALL_TURNS: [Turn; 3] = [Turn::Cw, Turn::Ccw, Turn::Half];

impl Face {
    pub fn axis(self) -> usize {
        match self {
            Face::R | Face::L => 0,
            Face::U | Face::D => 1,
            Face::F | Face::B => 2,
        }
    }

    pub fn sign(self) -> i32 {
        match self {
            Face::R | Face::U | Face::F => 1,
            Face::L | Face::D | Face::B => -1,
        }
    }
}

/// An atomic layer rotation.
///
/// `axis`  : 0=X, 1=Y, 2=Z
/// `layer` : -1, 0, +1 (which slice along the axis)
/// `turns` : signed quarter-turns around the +axis vector (right-hand rule).
///           +1 = 90° positive, -1 = 90° negative, +2 = 180°.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Move {
    pub axis: u8,
    pub layer: i8,
    pub turns: i8,
}

impl Move {
    pub fn slice(axis: u8, layer: i8, turns: i8) -> Self {
        Self { axis, layer, turns }
    }

    /// Build a Move from Singmaster face notation (U/D/L/R/F/B + Cw/Ccw/Half).
    pub fn face_turn(face: Face, turn: Turn) -> Self {
        // For Cw "as seen from outside the face", the rotation around the
        // outward axis is -90°. Outward = sign(face) * +axis_vec, so the
        // signed-turn around +axis_vec is -sign(face) * (+1 for Cw).
        let base: i8 = match turn {
            Turn::Cw => 1,
            Turn::Ccw => -1,
            Turn::Half => 2,
        };
        let turns = if turn == Turn::Half {
            2
        } else {
            -(face.sign() as i8) * base
        };
        Self {
            axis: face.axis() as u8,
            layer: face.sign() as i8,
            turns,
        }
    }

    fn axis_vec(self) -> Vec3 {
        match self.axis {
            0 => Vec3::X,
            1 => Vec3::Y,
            _ => Vec3::Z,
        }
    }

    pub fn quat(self, progress: f32) -> Quat {
        let angle = self.turns as f32 * FRAC_PI_2 * progress;
        Quat::from_axis_angle(self.axis_vec(), angle)
    }

    /// Inverse: a move that exactly undoes `self`.
    /// `Cube::apply(mv); Cube::apply(mv.inverse())` returns to the starting state.
    #[allow(dead_code)]
    pub fn inverse(self) -> Self {
        Self {
            axis: self.axis,
            layer: self.layer,
            turns: -self.turns,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Cubie {
    pub home: IVec3,
    pub pos: IVec3,
    pub orient: Quat,
}

#[derive(Clone, Debug)]
pub struct Cube {
    pub cubies: Vec<Cubie>,
}

impl Cube {
    pub fn solved() -> Self {
        let mut cubies = Vec::with_capacity(26);
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }
                    let p = IVec3::new(x, y, z);
                    cubies.push(Cubie {
                        home: p,
                        pos: p,
                        orient: Quat::IDENTITY,
                    });
                }
            }
        }
        Self { cubies }
    }

    pub fn reset(&mut self) {
        *self = Self::solved();
    }

    pub fn apply(&mut self, mv: Move) {
        let q = mv.quat(1.0);
        let axis = mv.axis as usize;
        let layer = mv.layer as i32;
        for cubie in self.cubies.iter_mut() {
            if cubie.pos[axis] == layer {
                let pos_f = cubie.pos.as_vec3();
                let new_pos = q * pos_f;
                cubie.pos = IVec3::new(
                    new_pos.x.round() as i32,
                    new_pos.y.round() as i32,
                    new_pos.z.round() as i32,
                );
                cubie.orient = (q * cubie.orient).normalize();
            }
        }
    }

    #[allow(dead_code)]
    pub fn apply_seq(&mut self, moves: &[Move]) {
        for &mv in moves {
            self.apply(mv);
        }
    }

    #[allow(dead_code)]
    pub fn is_solved(&self) -> bool {
        self.cubies
            .iter()
            .all(|c| c.pos == c.home && quat_approx_identity(c.orient))
    }
}

#[allow(dead_code)]
fn quat_approx_identity(q: Quat) -> bool {
    let dot = q.dot(Quat::IDENTITY).abs();
    (dot - 1.0).abs() < 1e-3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ft(face: Face, turn: Turn) -> Move {
        Move::face_turn(face, turn)
    }

    #[test]
    fn solved_state_is_solved() {
        assert!(Cube::solved().is_solved());
        assert_eq!(Cube::solved().cubies.len(), 26);
    }

    #[test]
    fn four_cw_turns_restore() {
        for face in ALL_FACES {
            let mut c = Cube::solved();
            for _ in 0..4 {
                c.apply(ft(face, Turn::Cw));
            }
            assert!(c.is_solved(), "face {:?} not restored after 4 CW", face);
        }
    }

    #[test]
    fn cw_then_ccw_restores() {
        for face in ALL_FACES {
            let mut c = Cube::solved();
            c.apply(ft(face, Turn::Cw));
            c.apply(ft(face, Turn::Ccw));
            assert!(c.is_solved(), "face {:?} not restored after Cw+Ccw", face);
        }
    }

    #[test]
    fn two_halves_restore() {
        for face in ALL_FACES {
            let mut c = Cube::solved();
            c.apply(ft(face, Turn::Half));
            c.apply(ft(face, Turn::Half));
            assert!(c.is_solved(), "face {:?} not restored after 2 halves", face);
        }
    }

    #[test]
    fn sexy_move_six_times_restores() {
        let seq = [
            ft(Face::R, Turn::Cw),
            ft(Face::U, Turn::Cw),
            ft(Face::R, Turn::Ccw),
            ft(Face::U, Turn::Ccw),
        ];
        let mut c = Cube::solved();
        for _ in 0..6 {
            c.apply_seq(&seq);
        }
        assert!(c.is_solved(), "sexy move x6 should restore solved state");
    }

    #[test]
    fn single_move_disturbs_state() {
        let mut c = Cube::solved();
        c.apply(ft(Face::R, Turn::Cw));
        assert!(!c.is_solved());
    }

    #[test]
    fn middle_slice_four_turns_restore() {
        // E slice (axis=Y, layer=0), 4 quarter-turns return to solved.
        let mut c = Cube::solved();
        for _ in 0..4 {
            c.apply(Move::slice(1, 0, 1));
        }
        assert!(c.is_solved());

        // M slice (axis=X, layer=0), 4 quarter-turns return to solved.
        let mut c = Cube::solved();
        for _ in 0..4 {
            c.apply(Move::slice(0, 0, 1));
        }
        assert!(c.is_solved());
    }

    #[test]
    fn inverse_undoes_any_move() {
        // For every face turn and every middle-slice turn, applying inverse
        // immediately after the move should restore the solved state.
        for face in ALL_FACES {
            for turn in ALL_TURNS {
                let mv = Move::face_turn(face, turn);
                let mut c = Cube::solved();
                c.apply(mv);
                c.apply(mv.inverse());
                assert!(c.is_solved(), "face_turn {:?} {:?} not undone", face, turn);
            }
        }
        for axis in 0u8..3 {
            for &layer in &[-1i8, 0, 1] {
                for &turns in &[-1i8, 1, 2] {
                    let mv = Move::slice(axis, layer, turns);
                    let mut c = Cube::solved();
                    c.apply(mv);
                    c.apply(mv.inverse());
                    assert!(c.is_solved(), "slice {:?} not undone", mv);
                }
            }
        }
    }

    #[test]
    fn reverse_inverse_history_solves_a_scrambled_cube() {
        // A handful of moves in random-ish order, then apply the
        // (reversed, inverted) sequence. Cube must end up solved.
        let history = [
            Move::face_turn(Face::R, Turn::Cw),
            Move::face_turn(Face::U, Turn::Cw),
            Move::face_turn(Face::F, Turn::Half),
            Move::slice(1, 0, 1), // E slice
            Move::face_turn(Face::L, Turn::Ccw),
            Move::face_turn(Face::B, Turn::Cw),
            Move::slice(0, 0, -1), // M slice reversed
        ];
        let mut c = Cube::solved();
        c.apply_seq(&history);
        assert!(!c.is_solved());
        let undo: Vec<Move> = history.iter().rev().map(|m| m.inverse()).collect();
        c.apply_seq(&undo);
        assert!(c.is_solved());
    }

    #[test]
    fn middle_slice_disturbs_state() {
        let mut c = Cube::solved();
        c.apply(Move::slice(1, 0, 1));
        assert!(!c.is_solved());
    }
}
