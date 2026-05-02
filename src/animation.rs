use crate::cube::{Cube, Move};
use std::collections::VecDeque;

pub const DEFAULT_DURATION: f32 = 0.15;
pub const SLOW_DURATION: f32 = 0.45;

pub struct Animator {
    queue: VecDeque<(Move, f32)>,
    current: Option<InProgress>,
}

#[derive(Copy, Clone)]
struct InProgress {
    mv: Move,
    elapsed: f32,
    duration: f32,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            current: None,
        }
    }

    /// Queue moves at the default (fast) duration. Used for scrambles and
    /// the basic numpad row/column rotations.
    pub fn push_many<I: IntoIterator<Item = Move>>(&mut self, moves: I) {
        self.queue
            .extend(moves.into_iter().map(|mv| (mv, DEFAULT_DURATION)));
    }

    /// Queue moves at the same explicit duration.
    pub fn push_many_with_duration<I: IntoIterator<Item = Move>>(
        &mut self,
        moves: I,
        duration: f32,
    ) {
        self.queue
            .extend(moves.into_iter().map(|mv| (mv, duration)));
    }

    /// Queue a single move with an explicit duration in seconds.
    pub fn push_one_with_duration(&mut self, mv: Move, duration: f32) {
        self.queue.push_back((mv, duration));
    }

    pub fn is_idle(&self) -> bool {
        self.queue.is_empty() && self.current.is_none()
    }

    pub fn tick(&mut self, dt: f32, cube: &mut Cube) {
        if self.current.is_none() {
            match self.queue.pop_front() {
                Some((mv, duration)) => {
                    self.current = Some(InProgress {
                        mv,
                        elapsed: 0.0,
                        duration,
                    });
                }
                None => return,
            }
        }
        let ip = self.current.as_mut().unwrap();
        ip.elapsed += dt;
        if ip.elapsed >= ip.duration {
            cube.apply(ip.mv);
            self.current = None;
        }
    }

    /// (move, progress in 0..1) for the currently-animating move, if any.
    pub fn current_frame(&self) -> Option<(Move, f32)> {
        self.current
            .map(|ip| (ip.mv, (ip.elapsed / ip.duration).clamp(0.0, 1.0)))
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.current = None;
    }
}
