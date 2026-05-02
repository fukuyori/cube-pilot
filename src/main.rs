use macroquad::prelude::*;

mod animation;
mod controls;
mod cube;
mod render;
mod scramble;
mod solver;

use animation::{Animator, SLOW_DURATION};
use controls::{col_move, row_move, ViewBasis};
use cube::{Cube, Move};
use render::{
    draw_cube_state, draw_front_face_labels, draw_pins, sticker_screen_center, world_to_screen_3d,
    Blink, OrbitCamera, PinAnchor,
};
use solver::AsyncSolver;

/// A solver result the user can step through. `next_index` points at the
/// next move that pressing Right would apply; `0..next_index` have already
/// been played.
struct Solution {
    moves: Vec<Move>,
    next_index: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MenuKind {
    Game,
    View,
    Help,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum UiAction {
    ScrambleOrStop,
    AutoShuffle,
    Solve,
    PrevStep,
    NextStep,
    PlayAll,
    Reset,
    ResetView,
    BlinkFront,
    ClearPins,
    Quit,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Cube Pilot".to_string(),
        window_width: 1024,
        window_height: 768,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut cube = Cube::solved();
    let mut animator = Animator::new();
    let mut camera = OrbitCamera::new();
    let mut rng = ::rand::thread_rng();
    let mut last_mouse: Option<Vec2> = None;
    let mut left_mouse_down_prev = false;
    let mut suppress_drag_until_mouse_up = false;
    let mut blink: Option<Blink> = None;
    let mut pins: Vec<PinAnchor> = Vec::new();
    // Every committed move is recorded here, kept around for diagnostics
    // (the visible move count). Cleared on Shift+R reset and on a successful solve.
    let mut history: Vec<Move> = Vec::new();
    let mut async_solver = AsyncSolver::new();
    // Solution returned by the solver, kept around so the user can step
    // through it forward / backward with the arrow keys.
    let mut solution: Option<Solution> = None;
    let mut auto_shuffle = false;
    let mut open_menu: Option<MenuKind> = None;
    // Tracks how many consecutive frames Shift has been seen as OFF.
    // The Windows "fake Shift" around Numpad presses flips Shift to off for
    // a single frame even at the GetAsyncKeyState level. By treating Shift
    // as still-held for a few frames after the last ON observation, we paper
    // over that one-frame glitch.
    let mut shift_off_frames: u32 = 999;
    const SHIFT_GRACE_FRAMES: u32 = 3;

    prevent_quit();
    loop {
        if is_key_pressed(KeyCode::Escape) || is_quit_requested() {
            break;
        }
        if is_key_pressed(KeyCode::Space) {
            apply_ui_action(
                UiAction::ScrambleOrStop,
                &mut cube,
                &mut animator,
                &mut camera,
                &mut rng,
                &mut history,
                &mut async_solver,
                &mut solution,
                &mut auto_shuffle,
                &mut blink,
                &mut pins,
                None,
            );
        }
        if is_key_pressed(KeyCode::S) {
            apply_ui_action(
                UiAction::Solve,
                &mut cube,
                &mut animator,
                &mut camera,
                &mut rng,
                &mut history,
                &mut async_solver,
                &mut solution,
                &mut auto_shuffle,
                &mut blink,
                &mut pins,
                None,
            );
        }
        if let Some(result) = async_solver.poll() {
            match result {
                Ok(moves) => {
                    solution = Some(Solution {
                        moves,
                        next_index: 0,
                    });
                }
                Err(e) => {
                    eprintln!("solver error: {e}");
                }
            }
        }
        // Step forward through the stored solution (slower than scramble
        // pace so each move is easy to see).
        if is_key_pressed(KeyCode::Right) && animator.is_idle() {
            if let Some(sol) = solution.as_mut() {
                if sol.next_index < sol.moves.len() {
                    auto_shuffle = false;
                    queue_next_solution_step(&mut animator, sol);
                }
            }
        }
        // Step backward — apply the inverse of the last-played move.
        if is_key_pressed(KeyCode::Left) && animator.is_idle() {
            if let Some(sol) = solution.as_mut() {
                if sol.next_index > 0 {
                    auto_shuffle = false;
                    queue_prev_solution_step(&mut animator, sol);
                }
            }
        }
        // Auto-play all remaining steps at the slow pace.
        if is_key_pressed(KeyCode::Down) && animator.is_idle() {
            if let Some(sol) = solution.as_mut() {
                auto_shuffle = false;
                queue_all_solution_steps(&mut animator, sol);
            }
        }
        if is_key_pressed(KeyCode::Kp5) {
            camera = OrbitCamera::new();
        }

        let view =
            ViewBasis::from_vectors(camera.pos_dir(), camera.right_world(), camera.up_world());
        let raw_shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        #[cfg(target_os = "windows")]
        let raw_shift = raw_shift || async_key_down(0x10);
        if raw_shift {
            shift_off_frames = 0;
        } else {
            shift_off_frames = shift_off_frames.saturating_add(1);
        }
        let shift_active = shift_off_frames < SHIFT_GRACE_FRAMES;
        let ctrl_held = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || raw_ctrl_down();

        if shift_active && is_key_pressed(KeyCode::R) {
            apply_ui_action(
                UiAction::Reset,
                &mut cube,
                &mut animator,
                &mut camera,
                &mut rng,
                &mut history,
                &mut async_solver,
                &mut solution,
                &mut auto_shuffle,
                &mut blink,
                &mut pins,
                None,
            );
        }
        if shift_active && is_key_pressed(KeyCode::A) {
            apply_ui_action(
                UiAction::AutoShuffle,
                &mut cube,
                &mut animator,
                &mut camera,
                &mut rng,
                &mut history,
                &mut async_solver,
                &mut solution,
                &mut auto_shuffle,
                &mut blink,
                &mut pins,
                None,
            );
        }
        if let Some(action) = handle_ui_input(&mut open_menu) {
            if action == UiAction::Quit {
                break;
            }
            apply_ui_action(
                action,
                &mut cube,
                &mut animator,
                &mut camera,
                &mut rng,
                &mut history,
                &mut async_solver,
                &mut solution,
                &mut auto_shuffle,
                &mut blink,
                &mut pins,
                Some(view),
            );
        }
        if auto_shuffle && animator.is_idle() {
            let s = scramble::generate(&mut rng, 25);
            history.extend_from_slice(&s);
            animator.push_many_with_duration(s, SLOW_DURATION);
        }

        if ctrl_held {
            apply_ctrl_view_command(&mut camera);
        } else if let Some(mv) = pressed_numpad_move(view, shift_active) {
            history.push(mv);
            animator.push_many(std::iter::once(mv));
            // Manual rotation desyncs us from the stored solution.
            auto_shuffle = false;
            solution = None;
        }
        if is_key_pressed(KeyCode::F) {
            blink = Some(Blink::new(view.front_axis, view.front_sign));
        }

        let current_mouse = Vec2::from(mouse_position());
        let left_mouse_down = raw_left_mouse_down();
        let left_mouse_pressed = is_mouse_button_pressed(MouseButton::Left)
            || (left_mouse_down && !left_mouse_down_prev);
        left_mouse_down_prev = left_mouse_down;
        if !left_mouse_down {
            suppress_drag_until_mouse_up = false;
        }

        let pointer_over_ui = is_pointer_over_ui(open_menu);
        let placing_pin =
            !pointer_over_ui && is_key_down(KeyCode::F) && left_mouse_pressed && animator.is_idle();
        if placing_pin {
            if let Some(pin) = pick_front_pin(&cube, &camera.camera(), view, current_mouse) {
                if !pins.contains(&pin) {
                    pins.push(pin);
                }
            }
            last_mouse = None;
            suppress_drag_until_mouse_up = true;
        }
        let ctrl_front_click = !pointer_over_ui && !placing_pin && ctrl_held && left_mouse_pressed;
        let clicked_front_view = if ctrl_front_click {
            pick_front_cell(&camera.camera(), view, current_mouse)
        } else {
            None
        };
        if let Some((col, row)) = clicked_front_view {
            apply_ctrl_front_cell_command(&mut camera, col, row);
            last_mouse = None;
            suppress_drag_until_mouse_up = true;
        }
        let clicking_front_move = !pointer_over_ui
            && !placing_pin
            && clicked_front_view.is_none()
            && !ctrl_held
            && left_mouse_pressed
            && animator.is_idle();
        let clicked_front_move = if clicking_front_move {
            pick_front_click_move(&camera.camera(), view, current_mouse, shift_active)
        } else {
            None
        };
        if let Some(mv) = clicked_front_move {
            history.push(mv);
            animator.push_many(std::iter::once(mv));
            auto_shuffle = false;
            solution = None;
            last_mouse = None;
            suppress_drag_until_mouse_up = true;
        }
        if pointer_over_ui {
            last_mouse = None;
        } else if !placing_pin
            && !suppress_drag_until_mouse_up
            && clicked_front_view.is_none()
            && clicked_front_move.is_none()
            && left_mouse_pressed
        {
            last_mouse = Some(current_mouse);
        }
        if !pointer_over_ui
            && !placing_pin
            && !suppress_drag_until_mouse_up
            && clicked_front_view.is_none()
            && clicked_front_move.is_none()
            && left_mouse_down
        {
            if let Some(last) = last_mouse {
                let delta = current_mouse - last;
                camera.orbit(delta.x * 0.005, -delta.y * 0.005);
            }
            last_mouse = Some(current_mouse);
        } else {
            last_mouse = None;
        }

        let dt = get_frame_time();
        camera.tick(dt);
        animator.tick(dt, &mut cube);
        if let Some(b) = blink.as_mut() {
            b.tick(dt);
            if !b.is_active() {
                blink = None;
            }
        }

        clear_background(Color::new(0.12, 0.12, 0.14, 1.0));
        let cam3d = camera.camera();
        set_camera(&cam3d);
        draw_cube_state(&cube, &animator, blink);
        draw_pins(&cube, &animator, &pins);
        set_default_camera();

        // Numpad-key labels on the front face while blinking.
        if let Some(b) = blink.as_ref() {
            if b.is_active() && view.front_axis == b.axis && view.front_sign == b.sign {
                draw_front_face_labels(
                    &cam3d,
                    view.front_axis,
                    view.front_sign,
                    view.right_axis,
                    view.right_sign,
                    view.up_axis,
                    view.up_sign,
                    BLACK,
                );
            }
        }

        draw_menu_and_toolbar(
            open_menu,
            auto_shuffle,
            async_solver.is_busy(),
            solution.as_ref(),
        );
        next_frame().await;
    }
}

fn apply_ui_action(
    action: UiAction,
    cube: &mut Cube,
    animator: &mut Animator,
    camera: &mut OrbitCamera,
    rng: &mut impl ::rand::Rng,
    history: &mut Vec<Move>,
    async_solver: &mut AsyncSolver,
    solution: &mut Option<Solution>,
    auto_shuffle: &mut bool,
    blink: &mut Option<Blink>,
    pins: &mut Vec<PinAnchor>,
    view: Option<ViewBasis>,
) {
    match action {
        UiAction::ScrambleOrStop => {
            if *auto_shuffle || !animator.is_idle() {
                *auto_shuffle = false;
                animator.clear();
            } else {
                let s = scramble::generate(rng, 25);
                history.extend_from_slice(&s);
                animator.push_many(s);
                *solution = None;
            }
        }
        UiAction::AutoShuffle => {
            if *auto_shuffle {
                *auto_shuffle = false;
                animator.clear();
            } else {
                *auto_shuffle = true;
                *solution = None;
            }
        }
        UiAction::Solve => {
            if animator.is_idle()
                && !async_solver.is_busy()
                && !cube.cubies.iter().all(|c| c.pos == c.home)
            {
                *auto_shuffle = false;
                async_solver.start(cube);
            }
        }
        UiAction::PrevStep => {
            if animator.is_idle() {
                if let Some(sol) = solution.as_mut() {
                    *auto_shuffle = false;
                    queue_prev_solution_step(animator, sol);
                }
            }
        }
        UiAction::NextStep => {
            if animator.is_idle() {
                if let Some(sol) = solution.as_mut() {
                    *auto_shuffle = false;
                    queue_next_solution_step(animator, sol);
                }
            }
        }
        UiAction::PlayAll => {
            if animator.is_idle() {
                if let Some(sol) = solution.as_mut() {
                    *auto_shuffle = false;
                    queue_all_solution_steps(animator, sol);
                }
            }
        }
        UiAction::Reset => {
            cube.reset();
            animator.clear();
            *auto_shuffle = false;
            history.clear();
            *solution = None;
        }
        UiAction::ResetView => {
            *camera = OrbitCamera::new();
        }
        UiAction::BlinkFront => {
            if let Some(view) = view {
                *blink = Some(Blink::new(view.front_axis, view.front_sign));
            }
        }
        UiAction::ClearPins => {
            pins.clear();
        }
        UiAction::Quit => {}
    }
}

fn pick_front_pin(
    cube: &Cube,
    camera: &Camera3D,
    view: ViewBasis,
    mouse: Vec2,
) -> Option<PinAnchor> {
    let mut best: Option<(f32, PinAnchor)> = None;
    let normal = axis_ivec(view.front_axis, view.front_sign);
    for row in -1..=1 {
        for col in -1..=1 {
            let mut p = [0i32; 3];
            p[view.front_axis as usize] = view.front_sign as i32;
            p[view.right_axis as usize] = col * view.right_sign as i32;
            p[view.up_axis as usize] = row * view.up_sign as i32;
            let pos = IVec3::new(p[0], p[1], p[2]);
            let screen = sticker_screen_center(camera, pos, normal);
            let dist = screen.distance(mouse);
            if dist > 58.0 {
                continue;
            }
            let Some(cubie) = cube.cubies.iter().find(|c| c.pos == pos) else {
                continue;
            };
            let body_dir = body_dir_for_world_normal(cubie.orient, normal);
            let pin = PinAnchor {
                home: cubie.home,
                dir: body_dir,
            };
            if best.map_or(true, |(best_dist, _)| dist < best_dist) {
                best = Some((dist, pin));
            }
        }
    }
    best.map(|(_, pin)| pin)
}

fn pick_front_click_move(
    camera: &Camera3D,
    view: ViewBasis,
    mouse: Vec2,
    modifier: bool,
) -> Option<Move> {
    let (col, row) = pick_front_cell(camera, view, mouse)?;
    front_cell_move(view, col, row, modifier)
}

fn pick_front_cell(camera: &Camera3D, view: ViewBasis, mouse: Vec2) -> Option<(i8, i8)> {
    let mut best: Option<(f32, i8, i8)> = None;
    for row in -1..=1 {
        for col in -1..=1 {
            let quad = front_cell_screen_quad(camera, view, col, row);
            if !point_in_quad(mouse, quad) {
                continue;
            }
            let center = (quad[0] + quad[1] + quad[2] + quad[3]) * 0.25;
            let dist = center.distance(mouse);
            if best.map_or(true, |(best_dist, _, _)| dist < best_dist) {
                best = Some((dist, col, row));
            }
        }
    }
    best.map(|(_, col, row)| (col, row))
}

fn front_cell_move(view: ViewBasis, col: i8, row: i8, modifier: bool) -> Option<Move> {
    if !modifier {
        match col {
            -1 => Some(row_move(view, row, false)),
            1 => Some(row_move(view, row, true)),
            _ => None,
        }
    } else {
        match row {
            -1 => Some(col_move(view, col, true)),
            1 => Some(col_move(view, col, false)),
            _ => None,
        }
    }
}

fn front_cell_screen_quad(camera: &Camera3D, view: ViewBasis, col: i8, row: i8) -> [Vec2; 4] {
    let mut corners = [Vec2::ZERO; 4];
    let offsets = [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
    for (i, (dx, dy)) in offsets.into_iter().enumerate() {
        let mut p = [0.0f32; 3];
        p[view.front_axis as usize] = view.front_sign as f32 * 1.485;
        p[view.right_axis as usize] = (col as f32 + dx) * view.right_sign as f32;
        p[view.up_axis as usize] = (row as f32 + dy) * view.up_sign as f32;
        corners[i] = world_to_screen_3d(camera, vec3(p[0], p[1], p[2]));
    }
    corners
}

fn point_in_quad(point: Vec2, quad: [Vec2; 4]) -> bool {
    point_in_triangle(point, quad[0], quad[1], quad[2])
        || point_in_triangle(point, quad[0], quad[2], quad[3])
}

fn point_in_triangle(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross2(point - a, b - a);
    let d2 = cross2(point - b, c - b);
    let d3 = cross2(point - c, a - c);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn cross2(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn apply_ctrl_front_cell_command(camera: &mut OrbitCamera, col: i8, row: i8) {
    const STEP: f32 = std::f32::consts::FRAC_PI_2;
    match (col, row) {
        (-1, 1) | (-1, 0) => camera.nudge_yaw(STEP),
        (1, 1) | (1, 0) => camera.nudge_yaw(-STEP),
        (0, 1) => camera.nudge_pitch(STEP),
        (0, -1) => camera.nudge_pitch(-STEP),
        _ => {}
    }
}

fn axis_ivec(axis: u8, sign: i8) -> IVec3 {
    match axis {
        0 => IVec3::new(sign as i32, 0, 0),
        1 => IVec3::new(0, sign as i32, 0),
        _ => IVec3::new(0, 0, sign as i32),
    }
}

fn body_dir_for_world_normal(orient: Quat, world_normal: IVec3) -> IVec3 {
    let body_n = orient.inverse() * world_normal.as_vec3();
    IVec3::new(
        body_n.x.round() as i32,
        body_n.y.round() as i32,
        body_n.z.round() as i32,
    )
}

fn queue_next_solution_step(animator: &mut Animator, sol: &mut Solution) {
    if sol.next_index < sol.moves.len() {
        let mv = sol.moves[sol.next_index];
        animator.push_one_with_duration(mv, SLOW_DURATION);
        sol.next_index += 1;
    }
}

fn queue_prev_solution_step(animator: &mut Animator, sol: &mut Solution) {
    if sol.next_index > 0 {
        sol.next_index -= 1;
        let mv = sol.moves[sol.next_index].inverse();
        animator.push_one_with_duration(mv, SLOW_DURATION);
    }
}

fn queue_all_solution_steps(animator: &mut Animator, sol: &mut Solution) {
    while sol.next_index < sol.moves.len() {
        let mv = sol.moves[sol.next_index];
        animator.push_one_with_duration(mv, SLOW_DURATION);
        sol.next_index += 1;
    }
}

const MENU_H: f32 = 24.0;
const TOOLBAR_Y: f32 = 26.0;
const TOOLBAR_H: f32 = 32.0;
const BUTTON_H: f32 = 26.0;
const DROP_ITEM_H: f32 = 24.0;

fn handle_ui_input(open_menu: &mut Option<MenuKind>) -> Option<UiAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }

    let mouse = Vec2::from(mouse_position());
    for (kind, rect, _) in menu_title_rects() {
        if rect.contains(mouse) {
            *open_menu = if *open_menu == Some(kind) {
                None
            } else {
                Some(kind)
            };
            return None;
        }
    }

    if let Some(kind) = *open_menu {
        for (rect, action, _) in menu_item_rects(kind) {
            if rect.contains(mouse) {
                *open_menu = None;
                return Some(action);
            }
        }
    }

    for (rect, action, _) in toolbar_rects() {
        if rect.contains(mouse) {
            *open_menu = None;
            return Some(action);
        }
    }

    *open_menu = None;
    None
}

fn is_pointer_over_ui(open_menu: Option<MenuKind>) -> bool {
    let mouse = Vec2::from(mouse_position());
    if mouse.y <= TOOLBAR_Y + TOOLBAR_H {
        return true;
    }
    if let Some(kind) = open_menu {
        return menu_item_rects(kind)
            .into_iter()
            .any(|(rect, _, _)| rect.contains(mouse));
    }
    false
}

fn draw_menu_and_toolbar(
    open_menu: Option<MenuKind>,
    auto_shuffle: bool,
    solver_busy: bool,
    solution: Option<&Solution>,
) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        TOOLBAR_Y + TOOLBAR_H,
        Color::new(0.07, 0.07, 0.08, 0.94),
    );
    draw_line(
        0.0,
        MENU_H,
        screen_width(),
        MENU_H,
        1.0,
        Color::new(0.22, 0.22, 0.24, 1.0),
    );
    draw_line(
        0.0,
        TOOLBAR_Y + TOOLBAR_H,
        screen_width(),
        TOOLBAR_Y + TOOLBAR_H,
        1.0,
        Color::new(0.24, 0.24, 0.26, 1.0),
    );

    for (kind, rect, label) in menu_title_rects() {
        let active = open_menu == Some(kind);
        let bg = if active {
            Color::new(0.22, 0.23, 0.27, 1.0)
        } else {
            Color::new(0.07, 0.07, 0.08, 0.0)
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
        draw_text(label, rect.x + 10.0, rect.y + 17.0, 16.0, LIGHTGRAY);
    }

    for (rect, action, label) in toolbar_rects() {
        let active = action == UiAction::AutoShuffle && auto_shuffle;
        let bg = if active {
            Color::new(0.95, 0.55, 0.18, 1.0)
        } else {
            Color::new(0.18, 0.19, 0.21, 1.0)
        };
        let fg = if active { BLACK } else { WHITE };
        draw_button(rect, label, bg, fg);
    }

    if let Some(sol) = solution {
        let txt = format!("{}/{}", sol.next_index, sol.moves.len());
        let dim = measure_text(&txt, None, 18, 1.0);
        draw_text(
            &txt,
            screen_width() - dim.width - 14.0,
            TOOLBAR_Y + 21.0,
            18.0,
            GREEN,
        );
    } else if solver_busy {
        draw_text(
            "SOLVING",
            screen_width() - 88.0,
            TOOLBAR_Y + 21.0,
            18.0,
            YELLOW,
        );
    }

    if let Some(kind) = open_menu {
        for (rect, _, label) in menu_item_rects(kind) {
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::new(0.12, 0.12, 0.14, 0.98),
            );
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                1.0,
                Color::new(0.28, 0.28, 0.31, 1.0),
            );
            draw_text(label, rect.x + 10.0, rect.y + 17.0, 15.0, WHITE);
        }
    }
}

fn draw_button(rect: Rect, label: &str, bg: Color, fg: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        Color::new(0.35, 0.35, 0.38, 1.0),
    );
    let dim = measure_text(label, None, 16, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - dim.width) * 0.5,
        rect.y + 18.0,
        16.0,
        fg,
    );
}

fn menu_title_rects() -> [(MenuKind, Rect, &'static str); 3] {
    [
        (MenuKind::Game, Rect::new(4.0, 0.0, 58.0, MENU_H), "Game"),
        (MenuKind::View, Rect::new(64.0, 0.0, 54.0, MENU_H), "View"),
        (MenuKind::Help, Rect::new(120.0, 0.0, 54.0, MENU_H), "Help"),
    ]
}

fn menu_item_rects(kind: MenuKind) -> Vec<(Rect, UiAction, &'static str)> {
    let (x, width, items): (f32, f32, Vec<(UiAction, &'static str)>) = match kind {
        MenuKind::Game => (
            4.0,
            132.0,
            vec![
                (UiAction::ScrambleOrStop, "Scramble / Stop"),
                (UiAction::AutoShuffle, "Auto Shuffle"),
                (UiAction::Solve, "Solve"),
                (UiAction::Reset, "Reset"),
                (UiAction::Quit, "Quit"),
            ],
        ),
        MenuKind::View => (
            64.0,
            116.0,
            vec![
                (UiAction::ResetView, "Reset View"),
                (UiAction::BlinkFront, "Front Labels"),
                (UiAction::ClearPins, "Clear Pins"),
            ],
        ),
        MenuKind::Help => (
            120.0,
            156.0,
            vec![
                (UiAction::PrevStep, "Solution Prev"),
                (UiAction::NextStep, "Solution Next"),
                (UiAction::PlayAll, "Solution Play"),
            ],
        ),
    };

    items
        .into_iter()
        .enumerate()
        .map(|(i, (action, label))| {
            (
                Rect::new(x, MENU_H + i as f32 * DROP_ITEM_H, width, DROP_ITEM_H),
                action,
                label,
            )
        })
        .collect()
}

fn toolbar_rects() -> [(Rect, UiAction, &'static str); 10] {
    let y = TOOLBAR_Y + 3.0;
    [
        (
            Rect::new(8.0, y, 44.0, BUTTON_H),
            UiAction::ScrambleOrStop,
            "Scr",
        ),
        (
            Rect::new(56.0, y, 48.0, BUTTON_H),
            UiAction::AutoShuffle,
            "Auto",
        ),
        (Rect::new(108.0, y, 42.0, BUTTON_H), UiAction::Solve, "Sol"),
        (Rect::new(160.0, y, 34.0, BUTTON_H), UiAction::PrevStep, "<"),
        (Rect::new(198.0, y, 34.0, BUTTON_H), UiAction::NextStep, ">"),
        (Rect::new(236.0, y, 38.0, BUTTON_H), UiAction::PlayAll, ">>"),
        (
            Rect::new(284.0, y, 54.0, BUTTON_H),
            UiAction::Reset,
            "Reset",
        ),
        (
            Rect::new(342.0, y, 46.0, BUTTON_H),
            UiAction::ResetView,
            "View",
        ),
        (
            Rect::new(392.0, y, 46.0, BUTTON_H),
            UiAction::BlinkFront,
            "Face",
        ),
        (
            Rect::new(442.0, y, 42.0, BUTTON_H),
            UiAction::ClearPins,
            "Clr",
        ),
    ]
}

// Whether Shift or Ctrl is currently physically held.
// On Windows we query GetAsyncKeyState directly because Windows synthesizes
// fake Shift up/down events around Numpad presses, which would otherwise
// momentarily flip macroquad's is_key_down state on the Numpad-press frame
// and cause Shift+Numpad to be misread as plain Numpad.
#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
}

#[cfg(target_os = "windows")]
fn async_key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16) & 0x8000 != 0 }
}

#[cfg(target_os = "windows")]
fn raw_left_mouse_down() -> bool {
    is_mouse_button_down(MouseButton::Left) || native_left_mouse_down()
}

#[cfg(not(target_os = "windows"))]
fn raw_left_mouse_down() -> bool {
    is_mouse_button_down(MouseButton::Left)
}

#[cfg(target_os = "windows")]
fn native_left_mouse_down() -> bool {
    async_key_down(0x01)
}

#[cfg(not(target_os = "windows"))]
fn native_left_mouse_down() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn raw_ctrl_down() -> bool {
    async_key_down(0x11) || async_key_down(0xA2) || async_key_down(0xA3)
}

#[cfg(not(target_os = "windows"))]
fn raw_ctrl_down() -> bool {
    false
}

/// Handle Ctrl + numpad view commands. Each press triggers a 90° rotation
/// of the camera about the corresponding axis, animated to the target via
/// the slerp in `OrbitCamera::tick`. Repeated presses keep accumulating —
/// e.g. two presses of Ctrl+Kp8 carries the camera over the top so U ends
/// up at the bottom of the screen.
///
///   7 / 4 → yaw left  (cube's left face rotates into front view)
///   9 / 6 → yaw right
///   8     → pitch up  (camera tilts up over the top)
///   2     → pitch down
fn apply_ctrl_view_command(camera: &mut OrbitCamera) {
    const STEP: f32 = std::f32::consts::FRAC_PI_2; // 90° per press

    if is_key_pressed(KeyCode::Kp7) || is_key_pressed(KeyCode::Kp4) {
        camera.nudge_yaw(STEP);
    } else if is_key_pressed(KeyCode::Kp9) || is_key_pressed(KeyCode::Kp6) {
        camera.nudge_yaw(-STEP);
    } else if is_key_pressed(KeyCode::Kp8) {
        camera.nudge_pitch(STEP);
    } else if is_key_pressed(KeyCode::Kp2) {
        camera.nudge_pitch(-STEP);
    }
}

/// Map numpad key presses to a layer rotation, relative to the current view.
/// `modifier` is the (Shift-grace-window) flag indicating whether to interpret
/// the press as a column rotation (true) or a row rotation (false).
fn pressed_numpad_move(view: ViewBasis, modifier: bool) -> Option<Move> {
    if !modifier {
        if is_key_pressed(KeyCode::Kp7) {
            return Some(row_move(view, 1, false));
        }
        if is_key_pressed(KeyCode::Kp4) {
            return Some(row_move(view, 0, false));
        }
        if is_key_pressed(KeyCode::Kp1) {
            return Some(row_move(view, -1, false));
        }
        if is_key_pressed(KeyCode::Kp9) {
            return Some(row_move(view, 1, true));
        }
        if is_key_pressed(KeyCode::Kp6) {
            return Some(row_move(view, 0, true));
        }
        if is_key_pressed(KeyCode::Kp3) {
            return Some(row_move(view, -1, true));
        }
    } else {
        if is_key_pressed(KeyCode::Kp1) {
            return Some(col_move(view, -1, true));
        }
        if is_key_pressed(KeyCode::Kp2) {
            return Some(col_move(view, 0, true));
        }
        if is_key_pressed(KeyCode::Kp3) {
            return Some(col_move(view, 1, true));
        }
        if is_key_pressed(KeyCode::Kp7) {
            return Some(col_move(view, -1, false));
        }
        if is_key_pressed(KeyCode::Kp8) {
            return Some(col_move(view, 0, false));
        }
        if is_key_pressed(KeyCode::Kp9) {
            return Some(col_move(view, 1, false));
        }
    }
    None
}
