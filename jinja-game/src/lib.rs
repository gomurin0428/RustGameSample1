use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, KeyboardEvent, Window};

const TILE_SIZE: f64 = 32.0;
const PLAYER_WIDTH: f64 = 26.0;
const PLAYER_HEIGHT: f64 = 30.0;
const MOVE_SPEED: f64 = 220.0;
const JUMP_SPEED: f64 = 440.0;
const GRAVITY: f64 = 1100.0;
const MAX_FALL_SPEED: f64 = 900.0;
const EPSILON: f64 = 0.001;
const LEVEL_MAP: &[&str] = &[
    "................................................",
    "................................................",
    "...............####.............................",
    ".............................#####..............",
    ".......................###......................",
    "..............####.................####.........",
    ".................##........###..................",
    "................##....................###.......",
    "....P.....###..........####.................S...",
    "###############....###########....##########....",
    "###############....###########....##########....",
];

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    bootstrap()
}

fn bootstrap() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;

    let canvas = resolve_canvas(&document)?;
    let context = canvas_context(&canvas)?;

    let game = Rc::new(RefCell::new(Game::new(canvas.width() as f64, canvas.height() as f64)));
    let input = Rc::new(RefCell::new(Input::default()));
    register_input_listeners(&document, Rc::clone(&input))?;

    start_animation_loop(window, context, game, input);

    Ok(())
}

fn resolve_canvas(document: &Document) -> Result<HtmlCanvasElement, JsValue> {
    Ok(
        document
            .get_element_by_id("game-canvas")
            .ok_or_else(|| JsValue::from_str("canvas element with id 'game-canvas' not found"))?
            .dyn_into::<HtmlCanvasElement>()?,
    )
}

fn canvas_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    Ok(
        canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("failed to acquire 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?,
    )
}
fn register_input_listeners(document: &Document, input: Rc<RefCell<Input>>) -> Result<(), JsValue> {
    {
        let input = Rc::clone(&input);
        let key_down = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            input.borrow_mut().press(event.key());
            event.prevent_default();
        }) as Box<dyn FnMut(_)>);

        document.add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())?;
        key_down.forget();
    }

    {
        let key_up = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            input.borrow_mut().release(&event.key());
            event.prevent_default();
        }) as Box<dyn FnMut(_)>);

        document.add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())?;
        key_up.forget();
    }

    Ok(())
}
fn start_animation_loop(
    window: Window,
    context: CanvasRenderingContext2d,
    game: Rc<RefCell<Game>>,
    input: Rc<RefCell<Input>>,
) {
    let context = Rc::new(context);
    let animation = Rc::new(RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let animation_for_assignment = Rc::clone(&animation);
    let animation_for_request = Rc::clone(&animation);

    let window_clone = window.clone();
    let game_clone = Rc::clone(&game);
    let input_clone = Rc::clone(&input);
    let context_clone = Rc::clone(&context);

    *animation_for_assignment.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        {
            let mut game = game_clone.borrow_mut();
            let mut input = input_clone.borrow_mut();
            let context_ref: &CanvasRenderingContext2d = context_clone.as_ref();
            game.tick(timestamp, &mut input, context_ref);
        }
        let _ = window_clone.request_animation_frame(
            animation_for_request
                .borrow()
                .as_ref()
                .expect("animation frame closure missing")
                .as_ref()
                .unchecked_ref(),
        );
    }) as Box<dyn FnMut(f64)>));

    let _ = window.request_animation_frame(
        animation
            .borrow()
            .as_ref()
            .expect("animation frame closure missing")
            .as_ref()
            .unchecked_ref(),
    );

    std::mem::forget(animation);
}
#[derive(Default)]
struct Input {
    pressed: HashSet<String>,
    just_pressed: HashSet<String>,
}

impl Input {
    fn press(&mut self, key: String) {
        let normalized = normalize_key(&key);
        if self.pressed.insert(normalized.clone()) {
            self.just_pressed.insert(normalized);
        }
    }

    fn release(&mut self, key: &str) {
        let normalized = normalize_key(key);
        self.pressed.remove(&normalized);
        self.just_pressed.remove(&normalized);
    }

    fn is_pressed(&self, key: &str) -> bool {
        let normalized = normalize_key(key);
        self.pressed.contains(&normalized)
    }

    fn just_pressed(&self, key: &str) -> bool {
        let normalized = normalize_key(key);
        self.just_pressed.contains(&normalized)
    }

    fn end_frame(&mut self) {
        self.just_pressed.clear();
    }
}
fn normalize_key(key: &str) -> String {
    let lower = key.to_lowercase();
    match lower.as_str() {
        " " | "spacebar" => "space".to_string(),
        other => other.to_string(),
    }
}
struct Game {
    width: f64,
    height: f64,
    level: Level,
    player: Player,
    camera_x: f64,
    last_frame_time: Option<f64>,
    elapsed: f64,
    goal_reached: bool,
    goal_reached_at: Option<f64>,
}

impl Game {
    fn new(width: f64, height: f64) -> Self {
        let level = Level::from_map(LEVEL_MAP);
        let (spawn_x, spawn_y) = level.spawn_position(PLAYER_WIDTH, PLAYER_HEIGHT);
        let mut game = Self {
            width,
            height,
            player: Player::new(spawn_x, spawn_y),
            level,
            camera_x: 0.0,
            last_frame_time: None,
            elapsed: 0.0,
            goal_reached: false,
            goal_reached_at: None,
        };
        game.update_camera();
        game
    }

    fn tick(&mut self, timestamp: f64, input: &mut Input, context: &CanvasRenderingContext2d) {
        let dt = if let Some(last) = self.last_frame_time {
            (timestamp - last) / 1000.0
        } else {
            0.0
        };

        self.last_frame_time = Some(timestamp);
        self.elapsed += dt;

        self.update(dt, input);
        self.draw(context);
        input.end_frame();
    }
    fn update(&mut self, dt: f64, input: &Input) {
        if self.goal_reached {
            self.player.stop();
            self.update_camera();
            return;
        }

        let left = input.is_pressed("arrowleft") || input.is_pressed("a");
        let right = input.is_pressed("arrowright") || input.is_pressed("d");
        let jump_requested = input.just_pressed("arrowup")
            || input.just_pressed("w")
            || input.just_pressed("space")
            || input.just_pressed("z");

        self.player.apply_horizontal_input(left, right);
        if jump_requested {
            self.player.jump();
        }

        self.player.apply_gravity(dt);
        self.player.resolve_horizontal(dt, &self.level);
        self.player.resolve_vertical(dt, &self.level);

        if self.level.shrine_reached(self.player.rect()) {
            self.goal_reached = true;
            self.goal_reached_at = Some(self.elapsed);
        }

        if self.player.rect().bottom() > self.level.pixel_height() + TILE_SIZE {
            panic!("player left the playable area vertically");
        }

        self.update_camera();
    }
    fn update_camera(&mut self) {
        let level_width = self.level.pixel_width();
        if level_width <= self.width {
            self.camera_x = 0.0;
            return;
        }

        let center = self.player.rect().center_x();
        let half = self.width * 0.5;
        let min = 0.0;
        let max = level_width - self.width;
        self.camera_x = (center - half).clamp(min, max);
    }
    fn draw(&self, context: &CanvasRenderingContext2d) {
        context.set_fill_style_str("#0f172a");
        context.fill_rect(0.0, 0.0, self.width, self.height);

        context.save();
        context
            .translate(-self.camera_x, 0.0)
            .expect("failed to translate for camera");

        self.draw_background(context);
        self.level.draw_tiles(context);
        self.draw_shrine(context);
        self.player.draw(context);

        context.restore();

        self.draw_hud(context);
    }
    fn draw_background(&self, context: &CanvasRenderingContext2d) {
        let ground_y = self.level.pixel_height();
        context.set_fill_style_str("#1e293b");
        context.fill_rect(0.0, ground_y, self.level.pixel_width(), self.height - ground_y);

        context.set_stroke_style_str("#334155");
        for hill in 0..6 {
            let base_x = hill as f64 * 280.0;
            context.begin_path();
            context.move_to(base_x, ground_y);
            let _ = context.quadratic_curve_to(base_x + 140.0, ground_y - 90.0, base_x + 280.0, ground_y);
            context.stroke();
        }
    }
    fn draw_shrine(&self, context: &CanvasRenderingContext2d) {
        let shrine_rect = self.level.shrine_rect();
        let base_x = shrine_rect.x + 4.0;
        let base_y = shrine_rect.y + shrine_rect.height;
        let post_width = 6.0;
        let beam_height = 8.0;

        context.set_fill_style_str("#dc2626");
        context.fill_rect(base_x, shrine_rect.y, post_width, shrine_rect.height);
        context.fill_rect(
            base_x + TILE_SIZE - post_width - 8.0,
            shrine_rect.y,
            post_width,
            shrine_rect.height,
        );
        context.fill_rect(
            shrine_rect.x - 6.0,
            shrine_rect.y - beam_height,
            TILE_SIZE + 12.0,
            beam_height,
        );

        context.set_fill_style_str("#f97316");
        context.fill_rect(
            shrine_rect.x - 4.0,
            shrine_rect.y - beam_height - 6.0,
            TILE_SIZE + 8.0,
            6.0,
        );

        context.set_fill_style_str("#f8fafc");
        context.fill_rect(base_x + 4.0, base_y - 10.0, TILE_SIZE - 24.0, 10.0);
    }
    fn draw_hud(&self, context: &CanvasRenderingContext2d) {
        context.set_fill_style_str("#e2e8f0");
        context.set_font("16px 'Segoe UI', sans-serif");
        let message = if let Some(clear_time) = self.goal_reached_at {
            format!("クリアタイム: {:.2} 秒", clear_time)
        } else {
            format!("経過時間: {:.2} 秒", self.elapsed)
        };
        context
            .fill_text(&message, 16.0, 28.0)
            .expect("failed to draw HUD time text");

        context.set_font("13px 'Segoe UI', sans-serif");
        let goal_hint = if self.goal_reached {
            "神社に到達しました！R を押して再読み込みすると再挑戦できます。"
        } else {
            "矢印キー / A D で移動、スペース / W / ↑ でジャンプ。"
        };
        context
            .fill_text(goal_hint, 16.0, 50.0)
            .expect("failed to draw HUD hint text");
    }
}
struct Player {
    rect: Rect,
    velocity_x: f64,
    velocity_y: f64,
    on_ground: bool,
}

impl Player {
    fn new(x: f64, y: f64) -> Self {
        Self {
            rect: Rect::new(x, y, PLAYER_WIDTH, PLAYER_HEIGHT),
            velocity_x: 0.0,
            velocity_y: 0.0,
            on_ground: false,
        }
    }

    fn rect(&self) -> Rect {
        self.rect
    }
    fn apply_horizontal_input(&mut self, left: bool, right: bool) {
        self.velocity_x = match (left, right) {
            (true, false) => -MOVE_SPEED,
            (false, true) => MOVE_SPEED,
            _ => 0.0,
        };
    }

    fn jump(&mut self) {
        if self.on_ground {
            self.velocity_y = -JUMP_SPEED;
            self.on_ground = false;
        }
    }

    fn apply_gravity(&mut self, dt: f64) {
        self.velocity_y = (self.velocity_y + GRAVITY * dt).min(MAX_FALL_SPEED);
    }
    fn resolve_horizontal(&mut self, dt: f64, level: &Level) {
        let dx = self.velocity_x * dt;
        self.rect.x += dx;

        let rect = self.rect();
        let min_row = level.row_index(rect.y);
        let max_row = level.row_index(rect.bottom() - EPSILON);

        if dx > 0.0 {
            let max_col = level.col_index(rect.right() - EPSILON);
            for row in min_row..=max_row {
                if level.tile_kind(max_col, row).is_solid() {
                    self.rect.x = (max_col as f64) * TILE_SIZE - self.rect.width - EPSILON;
                    self.velocity_x = 0.0;
                    break;
                }
            }
        } else if dx < 0.0 {
            let min_col = level.col_index(rect.x + EPSILON);
            for row in min_row..=max_row {
                if level.tile_kind(min_col, row).is_solid() {
                    self.rect.x = (min_col as f64 + 1.0) * TILE_SIZE + EPSILON;
                    self.velocity_x = 0.0;
                    break;
                }
            }
        }
    }
    fn resolve_vertical(&mut self, dt: f64, level: &Level) {
        let dy = self.velocity_y * dt;
        self.rect.y += dy;

        let rect = self.rect();
        let min_col = level.col_index(rect.x + EPSILON);
        let max_col = level.col_index(rect.right() - EPSILON);

        if dy > 0.0 {
            let max_row = level.row_index(rect.bottom() - EPSILON);
            for col in min_col..=max_col {
                if level.tile_kind(col, max_row).is_solid() {
                    self.rect.y = (max_row as f64) * TILE_SIZE - self.rect.height - EPSILON;
                    self.velocity_y = 0.0;
                    self.on_ground = true;
                    return;
                }
            }
            self.on_ground = false;
        } else if dy < 0.0 {
            let min_row = level.row_index(rect.y + EPSILON);
            for col in min_col..=max_col {
                if level.tile_kind(col, min_row).is_solid() {
                    self.rect.y = (min_row as f64 + 1.0) * TILE_SIZE + EPSILON;
                    self.velocity_y = 0.0;
                    return;
                }
            }
        }
    }

    fn stop(&mut self) {
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
    }

    fn draw(&self, context: &CanvasRenderingContext2d) {
        context.set_fill_style_str("#38bdf8");
        context.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height);

        context.set_fill_style_str("#0ea5e9");
        context.fill_rect(self.rect.x, self.rect.y, self.rect.width, 6.0);
    }
}
#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }

    fn center_x(&self) -> f64 {
        self.x + self.width * 0.5
    }

    fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.right() > other.x
            && self.y < other.y + other.height
            && self.bottom() > other.y
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Empty,
    Solid,
    Shrine,
}

impl Tile {
    fn is_solid(self) -> bool {
        matches!(self, Tile::Solid)
    }
}
struct Level {
    columns: usize,
    rows: usize,
    tiles: Vec<Tile>,
    spawn_tile: (usize, usize),
    shrine_tile: (usize, usize),
}
impl Level {
    fn from_map(raw: &[&str]) -> Self {
        if raw.is_empty() {
            panic!("level definition must contain at least one row");
        }

        let columns = raw[0].len();
        if columns == 0 {
            panic!("level definition row is empty");
        }

        let mut tiles = Vec::with_capacity(columns * raw.len());
        let mut spawn_tile = None;
        let mut shrine_tile = None;

        for (row_index, row) in raw.iter().enumerate() {
            if row.len() != columns {
                panic!("inconsistent row length detected in level definition");
            }

            for (col_index, ch) in row.chars().enumerate() {
                match ch {
                    '.' => tiles.push(Tile::Empty),
                    '#' | '=' => tiles.push(Tile::Solid),
                    'P' => {
                        spawn_tile = Some((col_index, row_index));
                        tiles.push(Tile::Empty);
                    }
                    'S' => {
                        shrine_tile = Some((col_index, row_index));
                        tiles.push(Tile::Shrine);
                    }
                    _ => panic!("unsupported tile symbol '{}' in level definition", ch),
                }
            }
        }

        let spawn_tile = spawn_tile.expect("level must specify a player spawn tile 'P'");
        let shrine_tile = shrine_tile.expect("level must specify a shrine tile 'S'");

        Self {
            columns,
            rows: raw.len(),
            tiles,
            spawn_tile,
            shrine_tile,
        }
    }
    fn pixel_width(&self) -> f64 {
        self.columns as f64 * TILE_SIZE
    }

    fn pixel_height(&self) -> f64 {
        self.rows as f64 * TILE_SIZE
    }

    fn spawn_position(&self, player_width: f64, player_height: f64) -> (f64, f64) {
        let (col, row) = self.spawn_tile;
        let x = col as f64 * TILE_SIZE + (TILE_SIZE - player_width) * 0.5;
        let y = row as f64 * TILE_SIZE + (TILE_SIZE - player_height);
        (x, y)
    }

    fn shrine_rect(&self) -> Rect {
        let (col, row) = self.shrine_tile;
        Rect::new(
            col as f64 * TILE_SIZE,
            row as f64 * TILE_SIZE - TILE_SIZE,
            TILE_SIZE,
            TILE_SIZE * 2.0,
        )
    }

    fn shrine_reached(&self, player_rect: Rect) -> bool {
        player_rect.intersects(&self.shrine_rect())
    }
    fn col_index(&self, world_x: f64) -> isize {
        (world_x / TILE_SIZE).floor() as isize
    }

    fn row_index(&self, world_y: f64) -> isize {
        (world_y / TILE_SIZE).floor() as isize
    }

    fn tile_kind(&self, col: isize, row: isize) -> Tile {
        if col < 0 || row < 0 || col >= self.columns as isize || row >= self.rows as isize {
            Tile::Solid
        } else {
            self.tiles[row as usize * self.columns + col as usize]
        }
    }

    fn draw_tiles(&self, context: &CanvasRenderingContext2d) {
        for row in 0..self.rows {
            for col in 0..self.columns {
                match self.tiles[row * self.columns + col] {
                    Tile::Solid => {
                        let x = col as f64 * TILE_SIZE;
                        let y = row as f64 * TILE_SIZE;
                        context.set_fill_style_str("#475569");
                        context.fill_rect(x, y, TILE_SIZE, TILE_SIZE);
                        context.set_fill_style_str("#64748b");
                        context.fill_rect(x, y, TILE_SIZE, 6.0);
                    }
                    Tile::Shrine => {}
                    Tile::Empty => {}
                }
            }
        }
    }
}
