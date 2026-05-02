use crate::cube::{Face, Move, ALL_FACES, ALL_TURNS};
use rand::seq::SliceRandom;
use rand::Rng;

/// Generate a WCA-flavored scramble of `len` moves.
///
/// Rules:
/// - No two consecutive moves on the same face.
/// - If the previous two moves shared an axis, the next move cannot share
///   that axis (avoids U-D-U redundancy).
pub fn generate<R: Rng + ?Sized>(rng: &mut R, len: usize) -> Vec<Move> {
    let mut faces: Vec<Face> = Vec::with_capacity(len);
    while faces.len() < len {
        let face = *ALL_FACES.choose(rng).unwrap();
        if let Some(&prev) = faces.last() {
            if prev == face {
                continue;
            }
            if faces.len() >= 2 {
                let prev2 = faces[faces.len() - 2];
                if prev.axis() == face.axis() && prev2.axis() == face.axis() {
                    continue;
                }
            }
        }
        faces.push(face);
    }
    faces
        .into_iter()
        .map(|face| {
            let turn = *ALL_TURNS.choose(rng).unwrap();
            Move::face_turn(face, turn)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn length_matches() {
        let mut rng = StdRng::seed_from_u64(42);
        let s = generate(&mut rng, 25);
        assert_eq!(s.len(), 25);
    }

    #[test]
    fn no_consecutive_same_layer() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..50 {
            let s = generate(&mut rng, 30);
            for w in s.windows(2) {
                assert!(!(w[0].axis == w[1].axis && w[0].layer == w[1].layer));
            }
        }
    }

    #[test]
    fn no_three_consecutive_same_axis() {
        let mut rng = StdRng::seed_from_u64(99);
        for _ in 0..50 {
            let s = generate(&mut rng, 40);
            for w in s.windows(3) {
                let same_axis = w[0].axis == w[1].axis && w[1].axis == w[2].axis;
                assert!(!same_axis, "three same-axis in a row: {:?}", w);
            }
        }
    }
}
