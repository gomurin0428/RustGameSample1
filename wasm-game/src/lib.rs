use std::cell::RefCell;
use std::collections::HashSet;
use std::f64::consts::{PI, SQRT_2};
use std::mem;
use std::rc::Rc;

use js_sys::Math;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, KeyboardEvent, Window};

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
    let context = canvas_2d_context(&canvas)?;

    let game = Rc::new(RefCell::new(Game::new(canvas.width() as f64, canvas.height() as f64)));
    let input = Rc::new(RefCell::new(Input::default()));
    register_input_listeners(&document, input.clone())?;

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

fn canvas_2d_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    Ok(
        canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("could not acquire 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?,
    )
}
fn register_input_listeners(document: &Document, input: Rc<RefCell<Input>>) -> Result<(), JsValue> {
    {
        let input = input.clone();
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
    let animation_for_assignment = animation.clone();
    let animation_for_request = animation.clone();

    let window_clone = window.clone();
    let game_clone = game.clone();
    let input_clone = input.clone();
    let ctx_for_loop = context.clone();

    *animation_for_assignment.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        {
            let mut game = game_clone.borrow_mut();
            let input = input_clone.borrow();
            game.tick(timestamp, &*input, &ctx_for_loop);
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

    mem::forget(animation);
}
#[derive(Default)]
struct Input {
    pressed: HashSet<String>,
}

impl Input {
    fn press(&mut self, key: String) {
        self.pressed.insert(Self::normalize(&key));
    }

    fn release(&mut self, key: &str) {
        self.pressed.remove(&Self::normalize(key));
    }

    fn is_pressed(&self, key: &str) -> bool {
        self.pressed.contains(&Self::normalize(key))
    }

    fn normalize(key: &str) -> String {
        key.to_lowercase()
    }
}

struct Game {
    width: f64,
    height: f64,
    player: Player,
    target: Target,
    score: u32,
    last_frame_time: Option<f64>,
}

impl Game {
    fn new(width: f64, height: f64) -> Self {
        let mut game = Self {
            width,
            height,
            player: Player::new(width * 0.5 - 15.0, height * 0.5 - 15.0, 30.0),
            target: Target::new(0.0, 0.0, 24.0),
            score: 0,
            last_frame_time: None,
        };
        game.reposition_target();
        game
    }

    fn tick(&mut self, timestamp: f64, input: &Input, context: &CanvasRenderingContext2d) {
        let dt = if let Some(last) = self.last_frame_time {
            (timestamp - last) / 1000.0
        } else {
            0.0
        };
        self.last_frame_time = Some(timestamp);

        self.update(dt, input);
        self.draw(context);
    }

    fn update(&mut self, dt: f64, input: &Input) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if input.is_pressed("arrowleft") || input.is_pressed("a") {
            dx -= 1.0;
        }
        if input.is_pressed("arrowright") || input.is_pressed("d") {
            dx += 1.0;
        }
        if input.is_pressed("arrowup") || input.is_pressed("w") {
            dy -= 1.0;
        }
        if input.is_pressed("arrowdown") || input.is_pressed("s") {
            dy += 1.0;
        }

        if dx != 0.0 && dy != 0.0 {
            let inv_sqrt_two = 1.0 / SQRT_2;
            dx *= inv_sqrt_two;
            dy *= inv_sqrt_two;
        }

        let speed = 220.0;
        self.player.x += dx * speed * dt;
        self.player.y += dy * speed * dt;

        self.player.x = self.player.x.clamp(0.0, self.width - self.player.size);
        self.player.y = self.player.y.clamp(0.0, self.height - self.player.size);

        if self.player.collides_with(&self.target) {
            self.score = self.score.saturating_add(1);
            self.reposition_target();
        }
    }

    fn draw(&self, context: &CanvasRenderingContext2d) {
        context.set_fill_style_str("#111827");
        context.fill_rect(0.0, 0.0, self.width, self.height);

        context.set_fill_style_str("#60a5fa");
        context.fill_rect(self.player.x, self.player.y, self.player.size, self.player.size);

        context.set_fill_style_str("#fbbf24");
        let (tx, ty) = self.target.center();
        context.begin_path();
        let _ = context.arc(tx, ty, self.target.size * 0.5, 0.0, PI * 2.0);
        context.fill();

        context.set_fill_style_str("#f9fafb");
        context.set_font("16px 'Segoe UI', sans-serif");
        let _ = context.fill_text(&format!("Score: {}", self.score), 16.0, 26.0);

        context.set_font("12px 'Segoe UI', sans-serif");
        let _ = context.fill_text("Use WASD or Arrow Keys to move", 16.0, self.height - 18.0);
    }

    fn reposition_target(&mut self) {
        let max_x = (self.width - self.target.size).max(0.0);
        let max_y = (self.height - self.target.size).max(0.0);
        self.target.x = Math::random() * max_x;
        self.target.y = Math::random() * max_y;
    }
}
struct Player {
    x: f64,
    y: f64,
    size: f64,
}

impl Player {
    fn new(x: f64, y: f64, size: f64) -> Self {
        Self { x, y, size }
    }

    fn collides_with(&self, target: &Target) -> bool {
        let half_size = self.size * 0.5;
        let player_center_x = self.x + half_size;
        let player_center_y = self.y + half_size;
        let (target_center_x, target_center_y) = target.center();
        let distance_sq =
            (player_center_x - target_center_x).powi(2) + (player_center_y - target_center_y).powi(2);
        let collision_radius = half_size + target.size * 0.5;

        distance_sq <= collision_radius.powi(2)
    }
}

struct Target {
    x: f64,
    y: f64,
    size: f64,
}

impl Target {
    fn new(x: f64, y: f64, size: f64) -> Self {
        Self { x, y, size }
    }

    fn center(&self) -> (f64, f64) {
        (self.x + self.size * 0.5, self.y + self.size * 0.5)
    }
}
