//! Kociemba two-phase solver integration.
//!
//! Wraps the `kociemba` crate so it can solve our cube state. The crate's
//! pruning tables are big and take a while to build the first time (they are
//! written to `tables/` next to the working directory and reused on later
//! runs), so the solve runs on a worker thread to keep the UI responsive.

use crate::cube::{Cube, Face, Move, Turn};
use kociemba::moves::Move as KMove;
use macroquad::prelude::IVec3;
use std::sync::mpsc;
use std::thread;

/// Render the current cube state into the 54-character URFDLB facelet string
/// expected by `kociemba::solver::solve`.
///
/// Solved state must produce
/// `"UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB"`.
pub fn cube_to_facelet_string(cube: &Cube) -> String {
    let mut s = String::with_capacity(54);
    for face_idx in 0..6 {
        for facelet_idx in 0..9 {
            let (pos, normal) = facelet_position(face_idx, facelet_idx);
            let cubie = cube
                .cubies
                .iter()
                .find(|c| c.pos == pos)
                .expect("no cubie at facelet position");
            let body_dir = body_dir_for_world_normal(cubie, normal);
            s.push(sticker_face_char(cubie.home, body_dir));
        }
    }
    s
}

/// World-space position and outward normal of facelet `(face_idx, facelet_idx)`
/// in URFDLB Kociemba notation.
fn facelet_position(face_idx: usize, facelet_idx: usize) -> (IVec3, IVec3) {
    let r = (facelet_idx / 3) as i32;
    let c = (facelet_idx % 3) as i32;
    match face_idx {
        // U: outward +Y. Row 0 (back of cube) → row 2 (front), col 0 (left) → col 2 (right).
        0 => (IVec3::new(c - 1, 1, r - 1), IVec3::new(0, 1, 0)),
        // R: outward +X. Row 0 (top) → row 2 (bot), col 0 (front) → col 2 (back).
        1 => (IVec3::new(1, 1 - r, 1 - c), IVec3::new(1, 0, 0)),
        // F: outward +Z. Row 0 (top) → row 2 (bot), col 0 (left) → col 2 (right).
        2 => (IVec3::new(c - 1, 1 - r, 1), IVec3::new(0, 0, 1)),
        // D: outward -Y. Row 0 (front) → row 2 (back), col 0 (left) → col 2 (right).
        3 => (IVec3::new(c - 1, -1, 1 - r), IVec3::new(0, -1, 0)),
        // L: outward -X. Row 0 (top) → row 2 (bot), col 0 (back) → col 2 (front).
        4 => (IVec3::new(-1, 1 - r, c - 1), IVec3::new(-1, 0, 0)),
        // B: outward -Z. Row 0 (top) → row 2 (bot), col 0 (right) → col 2 (left).
        5 => (IVec3::new(1 - c, 1 - r, -1), IVec3::new(0, 0, -1)),
        _ => unreachable!(),
    }
}

/// Map a world-space outward normal to the cubie's body-frame direction
/// (one of the six axis vectors).
fn body_dir_for_world_normal(cubie: &crate::cube::Cubie, world_normal: IVec3) -> IVec3 {
    let world_n = world_normal.as_vec3();
    let body_n = cubie.orient.inverse() * world_n;
    IVec3::new(
        body_n.x.round() as i32,
        body_n.y.round() as i32,
        body_n.z.round() as i32,
    )
}

/// Look up which face-color a sticker carries given the cubie's home position
/// and the body-frame direction of the sticker.
fn sticker_face_char(home: IVec3, dir: IVec3) -> char {
    if dir.x != 0 && home.x == dir.x {
        return if dir.x > 0 { 'R' } else { 'L' };
    }
    if dir.y != 0 && home.y == dir.y {
        return if dir.y > 0 { 'U' } else { 'D' };
    }
    if dir.z != 0 && home.z == dir.z {
        return if dir.z > 0 { 'F' } else { 'B' };
    }
    panic!("sticker: home={home:?} dir={dir:?} mismatch");
}

fn convert_kmove(km: KMove) -> Move {
    match km {
        KMove::U => Move::face_turn(Face::U, Turn::Cw),
        KMove::U2 => Move::face_turn(Face::U, Turn::Half),
        KMove::U3 => Move::face_turn(Face::U, Turn::Ccw),
        KMove::R => Move::face_turn(Face::R, Turn::Cw),
        KMove::R2 => Move::face_turn(Face::R, Turn::Half),
        KMove::R3 => Move::face_turn(Face::R, Turn::Ccw),
        KMove::F => Move::face_turn(Face::F, Turn::Cw),
        KMove::F2 => Move::face_turn(Face::F, Turn::Half),
        KMove::F3 => Move::face_turn(Face::F, Turn::Ccw),
        KMove::D => Move::face_turn(Face::D, Turn::Cw),
        KMove::D2 => Move::face_turn(Face::D, Turn::Half),
        KMove::D3 => Move::face_turn(Face::D, Turn::Ccw),
        KMove::L => Move::face_turn(Face::L, Turn::Cw),
        KMove::L2 => Move::face_turn(Face::L, Turn::Half),
        KMove::L3 => Move::face_turn(Face::L, Turn::Ccw),
        KMove::B => Move::face_turn(Face::B, Turn::Cw),
        KMove::B2 => Move::face_turn(Face::B, Turn::Half),
        KMove::B3 => Move::face_turn(Face::B, Turn::Ccw),
    }
}

/// Async wrapper around `kociemba::solver::solve` that runs on a worker thread
/// so the UI thread keeps drawing while the algorithm searches (and on the
/// very first call, while it builds its pruning tables).
pub struct AsyncSolver {
    rx: Option<mpsc::Receiver<Result<Vec<Move>, String>>>,
}

impl AsyncSolver {
    pub fn new() -> Self {
        Self { rx: None }
    }

    pub fn is_busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Kick off a solve for the given cube on a background thread. No-op if
    /// the solver is already busy.
    pub fn start(&mut self, cube: &Cube) {
        if self.is_busy() {
            return;
        }
        let facelets = cube_to_facelet_string(cube);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let res = kociemba::solver::solve(&facelets, 22, 5.0)
                .map_err(|e| format!("{e:?}"))
                .map(|sr| sr.solution.into_iter().map(convert_kmove).collect());
            let _ = tx.send(res);
        });
        self.rx = Some(rx);
    }

    /// Returns the solver result if one is ready this frame.
    pub fn poll(&mut self) -> Option<Result<Vec<Move>, String>> {
        if let Some(rx) = self.rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => {
                    self.rx = None;
                    Some(result)
                }
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    Some(Err("solver thread disconnected".to_string()))
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_cube_serializes_to_canonical_facelets() {
        let cube = Cube::solved();
        let s = cube_to_facelet_string(&cube);
        assert_eq!(s, "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB",);
    }

    #[test]
    fn applying_then_inverting_returns_canonical_facelets() {
        let mut cube = Cube::solved();
        cube.apply(Move::face_turn(Face::R, Turn::Cw));
        cube.apply(Move::face_turn(Face::U, Turn::Cw));
        cube.apply(Move::face_turn(Face::F, Turn::Half));
        // Reverse and invert.
        let undo = vec![
            Move::face_turn(Face::F, Turn::Half),
            Move::face_turn(Face::U, Turn::Ccw),
            Move::face_turn(Face::R, Turn::Ccw),
        ];
        cube.apply_seq(&undo);
        let s = cube_to_facelet_string(&cube);
        assert_eq!(s, "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB",);
    }

    #[test]
    fn single_r_move_changes_specific_facelets() {
        // After R move (CW from outside), the R face's stickers permute, and
        // certain other facelets change. Just check that the string is no
        // longer the canonical solved string.
        let mut cube = Cube::solved();
        cube.apply(Move::face_turn(Face::R, Turn::Cw));
        let s = cube_to_facelet_string(&cube);
        assert_ne!(s, "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB",);
        // R face's center (facelet 9+4 = 13) must still be R.
        assert_eq!(s.as_bytes()[13], b'R');
        // Each face's center is invariant — verify all six.
        assert_eq!(s.as_bytes()[4], b'U'); // U center
        assert_eq!(s.as_bytes()[13], b'R');
        assert_eq!(s.as_bytes()[22], b'F');
        assert_eq!(s.as_bytes()[31], b'D');
        assert_eq!(s.as_bytes()[40], b'L');
        assert_eq!(s.as_bytes()[49], b'B');
    }
}
