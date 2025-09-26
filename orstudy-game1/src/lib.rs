use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlButtonElement, HtmlCanvasElement,
    HtmlInputElement, HtmlSelectElement, Window,
};

const CANVAS_BG: &str = "#020617";
const CUSTOMER_COLOR: &str = "#38bdf8";
const WAITING_COLOR: &str = "#0ea5e9";
const DROPPED_COLOR: &str = "#f87171";
const SERVER_IDLE_COLOR: &str = "#1e293b";
const SERVER_BUSY_COLOR: &str = "#22c55e";
const TEXT_PRIMARY: &str = "#e2e8f0";
const TEXT_SECONDARY: &str = "#94a3b8";
const MAX_DT: f64 = 0.25;
const HISTORY_INTERVAL: f64 = 0.25;
const HISTORY_MAX_SAMPLES: usize = 360;

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

    let runtime = Rc::new(RefCell::new(AppRuntime::new(
        canvas.width() as f64,
        canvas.height() as f64,
    )?));
    register_ui(&document, Rc::clone(&runtime))?;

    start_animation_loop(window, context, runtime);
    Ok(())
}

fn resolve_canvas(document: &Document) -> Result<HtmlCanvasElement, JsValue> {
    document
        .get_element_by_id("game-canvas")
        .ok_or_else(|| JsValue::from_str("canvas element with id 'game-canvas' not found"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("failed to cast element to HtmlCanvasElement"))
}

fn canvas_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("failed to get 2d context"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("failed to cast context to CanvasRenderingContext2d"))
}
struct AppRuntime {
    simulation: Simulation,
    renderer: Renderer,
    last_frame_time: Option<f64>,
    defaults: Config,
}

impl AppRuntime {
    fn new(width: f64, height: f64) -> Result<Self, JsValue> {
        let defaults = Config::default();
        let simulation = Simulation::new(width, height, defaults.clone())?;
        Ok(Self {
            simulation,
            renderer: Renderer::new(width, height),
            last_frame_time: None,
            defaults,
        })
    }

    fn tick(&mut self, timestamp: f64, context: &CanvasRenderingContext2d) {
        let dt = if let Some(last) = self.last_frame_time {
            ((timestamp - last) / 1000.0).min(MAX_DT)
        } else {
            0.0
        };
        self.last_frame_time = Some(timestamp);
        if dt > 0.0 {
            self.simulation.update(dt);
        }
        self.renderer.draw(context, &self.simulation);
    }

    fn reset(&mut self) -> Result<(), JsValue> {
        self.simulation = Simulation::new(
            self.renderer.width,
            self.renderer.height,
            self.defaults.clone(),
        )?;
        self.last_frame_time = None;
        Ok(())
    }

    fn set_arrival_rate(&mut self, per_minute: f64) -> Result<(), JsValue> {
        self.simulation.set_arrival_rate(per_minute)?;
        Ok(())
    }

    fn set_service_rate(&mut self, per_minute: f64) -> Result<(), JsValue> {
        self.simulation.set_service_rate(per_minute)?;
        Ok(())
    }

    fn set_server_count(&mut self, count: usize) -> Result<(), JsValue> {
        self.simulation.set_server_count(count)?;
        Ok(())
    }

    fn set_queue_capacity(&mut self, capacity: usize) -> Result<(), JsValue> {
        self.simulation.set_queue_capacity(capacity)?;
        Ok(())
    }

    fn set_arrival_pattern(&mut self, pattern: ArrivalPattern) -> Result<(), JsValue> {
        self.simulation.set_arrival_pattern(pattern)?;
        Ok(())
    }

    fn set_service_pattern(&mut self, pattern: ServicePattern) -> Result<(), JsValue> {
        self.simulation.set_service_pattern(pattern)?;
        Ok(())
    }
}
fn start_animation_loop(
    window: Window,
    context: CanvasRenderingContext2d,
    runtime: Rc<RefCell<AppRuntime>>,
) {
    let context = Rc::new(context);
    let animation_handle = Rc::new(RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let animation_for_assignment = Rc::clone(&animation_handle);
    let animation_for_request = Rc::clone(&animation_handle);
    let runtime_for_tick = Rc::clone(&runtime);
    let context_for_tick = Rc::clone(&context);
    let window_for_tick = window.clone();

    *animation_for_assignment.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        {
            let mut runtime = runtime_for_tick.borrow_mut();
            runtime.tick(timestamp, context_for_tick.as_ref());
        }
        let _ = window_for_tick.request_animation_frame(
            animation_for_request
                .borrow()
                .as_ref()
                .expect("animation frame callback missing")
                .as_ref()
                .unchecked_ref(),
        );
    }) as Box<dyn FnMut(f64)>));

    let _ = window.request_animation_frame(
        animation_handle
            .borrow()
            .as_ref()
            .expect("animation frame callback missing")
            .as_ref()
            .unchecked_ref(),
    );

    std::mem::forget(animation_handle);
}
fn cast_element<T>(document: &Document, id: &str) -> Result<T, JsValue>
where
    T: JsCast,
{
    document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("element '{}' not found", id)))?
        .dyn_into::<T>()
        .map_err(|_| JsValue::from_str(&format!("failed to cast element '{}'", id)))
}
fn register_ui(document: &Document, runtime: Rc<RefCell<AppRuntime>>) -> Result<(), JsValue> {
    let arrival_slider: HtmlInputElement = cast_element(document, "arrival-rate")?;
    let service_slider: HtmlInputElement = cast_element(document, "service-rate")?;
    let server_slider: HtmlInputElement = cast_element(document, "server-count")?;
    let capacity_slider: HtmlInputElement = cast_element(document, "queue-capacity")?;
    let arrival_select: HtmlSelectElement = cast_element(document, "arrival-variance")?;
    let service_select: HtmlSelectElement = cast_element(document, "service-variance")?;
    let reset_button: HtmlButtonElement = cast_element(document, "reset-button")?;

    sync_all_labels(
        document,
        &runtime.borrow(),
        &arrival_slider,
        &service_slider,
        &server_slider,
        &capacity_slider,
        &arrival_select,
        &service_select,
    )?;

    attach_slider_handler(
        document.clone(),
        Rc::clone(&runtime),
        arrival_slider.clone(),
        "arrival-rate-value",
        |runtime, value| runtime.set_arrival_rate(value),
        |value| format!("{value:.0}"),
    );

    attach_slider_handler(
        document.clone(),
        Rc::clone(&runtime),
        service_slider.clone(),
        "service-rate-value",
        |runtime, value| runtime.set_service_rate(value),
        |value| format!("{value:.0}"),
    );

    attach_slider_handler(
        document.clone(),
        Rc::clone(&runtime),
        server_slider.clone(),
        "server-count-value",
        |runtime, value| runtime.set_server_count(value.round() as usize),
        |value| format!("{value:.0}"),
    );

    attach_slider_handler(
        document.clone(),
        Rc::clone(&runtime),
        capacity_slider.clone(),
        "queue-capacity-value",
        |runtime, value| runtime.set_queue_capacity(value.round() as usize),
        |value| format!("{value:.0}"),
    );

    attach_select_handler::<ArrivalPattern, _>(
        document.clone(),
        Rc::clone(&runtime),
        arrival_select.clone(),
        "arrival-variance-value",
        |runtime, value| runtime.set_arrival_pattern(value),
    );

    attach_select_handler::<ServicePattern, _>(
        document.clone(),
        Rc::clone(&runtime),
        service_select.clone(),
        "service-variance-value",
        |runtime, value| runtime.set_service_pattern(value),
    );

    attach_reset_handler(
        document.clone(),
        runtime,
        reset_button,
        arrival_slider,
        service_slider,
        server_slider,
        capacity_slider,
        arrival_select,
        service_select,
    );

    Ok(())
}
fn sync_all_labels(
    document: &Document,
    runtime: &AppRuntime,
    arrival_slider: &HtmlInputElement,
    service_slider: &HtmlInputElement,
    server_slider: &HtmlInputElement,
    capacity_slider: &HtmlInputElement,
    arrival_select: &HtmlSelectElement,
    service_select: &HtmlSelectElement,
) -> Result<(), JsValue> {
    update_text(
        document,
        "arrival-rate-value",
        &format!("{}", arrival_slider.value()),
    )?;
    update_text(
        document,
        "service-rate-value",
        &format!("{}", service_slider.value()),
    )?;
    update_text(
        document,
        "server-count-value",
        &format!("{}", server_slider.value()),
    )?;
    update_text(
        document,
        "queue-capacity-value",
        &format!("{}", capacity_slider.value()),
    )?;

    let arrival_pattern = runtime.simulation.config().arrival_pattern;
    update_text(document, "arrival-variance-value", arrival_pattern.label())?;

    let service_pattern = runtime.simulation.config().service_pattern;
    update_text(document, "service-variance-value", service_pattern.label())?;

    arrival_select.set_value(arrival_pattern.value_key());
    service_select.set_value(service_pattern.value_key());

    Ok(())
}
fn update_text(document: &Document, id: &str, text: &str) -> Result<(), JsValue> {
    let element = document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("element '{}' not found", id)))?;
    element.set_text_content(Some(text));
    Ok(())
}
fn attach_slider_handler<F, L>(
    document: Document,
    runtime: Rc<RefCell<AppRuntime>>,
    slider: HtmlInputElement,
    label_id: &str,
    apply: F,
    label_text: L,
) where
    F: Fn(&mut AppRuntime, f64) -> Result<(), JsValue> + 'static,
    L: Fn(f64) -> String + 'static,
{
    let label_id = label_id.to_string();
    let document_clone = document.clone();
    let runtime_clone = Rc::clone(&runtime);
    let slider_clone = slider.clone();

    let closure = Closure::wrap(Box::new(move |_event: Event| {
        let value = slider_clone
            .value()
            .parse::<f64>()
            .expect("failed to parse slider value to f64");
        {
            let mut runtime_ref = runtime_clone.borrow_mut();
            apply(&mut runtime_ref, value).expect("failed to apply slider change");
        }
        let text = label_text(value);
        document_clone
            .get_element_by_id(&label_id)
            .expect("label element missing for slider")
            .set_text_content(Some(&text));
    }) as Box<dyn FnMut(Event)>);

    slider
        .add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())
        .expect("failed to add slider event listener");
    closure.forget();
}
fn attach_select_handler<T, F>(
    document: Document,
    runtime: Rc<RefCell<AppRuntime>>,
    select: HtmlSelectElement,
    label_id: &str,
    apply: F,
) where
    T: PatternChoice + 'static,
    F: Fn(&mut AppRuntime, T) -> Result<(), JsValue> + 'static,
{
    let label_id = label_id.to_string();
    let document_clone = document.clone();
    let runtime_clone = Rc::clone(&runtime);
    let select_clone = select.clone();

    let closure = Closure::wrap(Box::new(move |_event: Event| {
        let key = select_clone.value();
        let pattern = T::from_key(&key).expect("invalid pattern key received from select element");
        {
            let mut runtime_ref = runtime_clone.borrow_mut();
            apply(&mut runtime_ref, pattern).expect("failed to apply select change");
        }
        document_clone
            .get_element_by_id(&label_id)
            .expect("label element missing for select")
            .set_text_content(Some(pattern.label()));
    }) as Box<dyn FnMut(Event)>);

    select
        .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
        .expect("failed to add select change listener");
    closure.forget();
}
fn attach_reset_handler(
    document: Document,
    runtime: Rc<RefCell<AppRuntime>>,
    reset_button: HtmlButtonElement,
    arrival_slider: HtmlInputElement,
    service_slider: HtmlInputElement,
    server_slider: HtmlInputElement,
    capacity_slider: HtmlInputElement,
    arrival_select: HtmlSelectElement,
    service_select: HtmlSelectElement,
) {
    let closure = Closure::wrap(Box::new(move |_event: Event| {
        {
            let mut runtime_ref = runtime.borrow_mut();
            runtime_ref.reset().expect("failed to reset simulation");
        }
        let defaults = Config::default();

        arrival_slider.set_value(&format!("{:.0}", defaults.arrival_rate_per_min));
        document
            .get_element_by_id("arrival-rate-value")
            .expect("arrival rate label missing during reset")
            .set_text_content(Some(&format!("{:.0}", defaults.arrival_rate_per_min)));

        service_slider.set_value(&format!("{:.0}", defaults.service_rate_per_min));
        document
            .get_element_by_id("service-rate-value")
            .expect("service rate label missing during reset")
            .set_text_content(Some(&format!("{:.0}", defaults.service_rate_per_min)));

        server_slider.set_value(&format!("{}", defaults.server_count));
        document
            .get_element_by_id("server-count-value")
            .expect("server count label missing during reset")
            .set_text_content(Some(&format!("{}", defaults.server_count)));

        capacity_slider.set_value(&format!("{}", defaults.queue_capacity));
        document
            .get_element_by_id("queue-capacity-value")
            .expect("queue capacity label missing during reset")
            .set_text_content(Some(&format!("{}", defaults.queue_capacity)));

        arrival_select.set_value(defaults.arrival_pattern.value_key());
        document
            .get_element_by_id("arrival-variance-value")
            .expect("arrival pattern label missing during reset")
            .set_text_content(Some(defaults.arrival_pattern.label()));

        service_select.set_value(defaults.service_pattern.value_key());
        document
            .get_element_by_id("service-variance-value")
            .expect("service pattern label missing during reset")
            .set_text_content(Some(defaults.service_pattern.label()));
    }) as Box<dyn FnMut(Event)>);

    reset_button
        .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .expect("failed to add reset button handler");
    closure.forget();
}
trait PatternChoice: Copy {
    fn from_key(key: &str) -> Result<Self, JsValue>
    where
        Self: Sized;
    fn value_key(self) -> &'static str;
    fn label(self) -> &'static str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrivalPattern {
    Deterministic,
    Poisson,
    Bursty,
}

impl PatternChoice for ArrivalPattern {
    fn from_key(key: &str) -> Result<Self, JsValue> {
        match key {
            "deterministic" => Ok(Self::Deterministic),
            "poisson" => Ok(Self::Poisson),
            "bursty" => Ok(Self::Bursty),
            other => Err(JsValue::from_str(&format!(
                "unsupported arrival pattern '{}'",
                other
            ))),
        }
    }

    fn value_key(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Poisson => "poisson",
            Self::Bursty => "bursty",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Deterministic => "規則的",
            Self::Poisson => "ポアソン",
            Self::Bursty => "集中到着",
        }
    }
}

impl Default for ArrivalPattern {
    fn default() -> Self {
        Self::Poisson
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServicePattern {
    Deterministic,
    Exponential,
    Erlang,
}

impl PatternChoice for ServicePattern {
    fn from_key(key: &str) -> Result<Self, JsValue> {
        match key {
            "deterministic" => Ok(Self::Deterministic),
            "exponential" => Ok(Self::Exponential),
            "erlang" => Ok(Self::Erlang),
            other => Err(JsValue::from_str(&format!(
                "unsupported service pattern '{}'",
                other
            ))),
        }
    }

    fn value_key(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Exponential => "exponential",
            Self::Erlang => "erlang",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Deterministic => "一定",
            Self::Exponential => "指数分布",
            Self::Erlang => "エルラン",
        }
    }
}

impl Default for ServicePattern {
    fn default() -> Self {
        Self::Exponential
    }
}
#[derive(Clone)]
struct Config {
    arrival_rate_per_min: f64,
    service_rate_per_min: f64,
    server_count: usize,
    queue_capacity: usize,
    arrival_pattern: ArrivalPattern,
    service_pattern: ServicePattern,
}

impl Config {
    fn arrival_rate_per_sec(&self) -> f64 {
        self.arrival_rate_per_min / 60.0
    }

    fn service_rate_per_sec(&self) -> f64 {
        self.service_rate_per_min / 60.0
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            arrival_rate_per_min: 12.0,
            service_rate_per_min: 18.0,
            server_count: 2,
            queue_capacity: 25,
            arrival_pattern: ArrivalPattern::Poisson,
            service_pattern: ServicePattern::Exponential,
        }
    }
}
struct Simulation {
    config: Config,
    queue: VecDeque<Customer>,
    servers: Vec<Option<ServerState>>,
    stats: Stats,
    elapsed: f64,
    time_since_arrival: f64,
    next_arrival_in: f64,
    history: VecDeque<HistorySample>,
    history_timer: f64,
    next_customer_id: u64,
}

#[derive(Clone, Copy)]
struct Customer {
    id: u64,
    arrival_at: f64,
}

#[derive(Clone, Copy)]
struct ServerState {
    customer: Customer,
    service_remaining: f64,
    service_total: f64,
    started_at: f64,
}

struct HistorySample {
    time: f64,
    queue_len: usize,
    in_system: usize,
}

struct Stats {
    arrivals: u64,
    served: u64,
    dropped: u64,
    total_wait_time: f64,
    total_system_time: f64,
    queue_area: f64,
    busy_time_by_server: Vec<f64>,
    peak_queue: usize,
}
impl Stats {
    fn new(server_count: usize) -> Self {
        Self {
            arrivals: 0,
            served: 0,
            dropped: 0,
            total_wait_time: 0.0,
            total_system_time: 0.0,
            queue_area: 0.0,
            busy_time_by_server: vec![0.0; server_count],
            peak_queue: 0,
        }
    }

    fn resize_servers(&mut self, server_count: usize) {
        self.busy_time_by_server.resize(server_count, 0.0);
    }
}
impl Simulation {
    fn new(_width: f64, _height: f64, config: Config) -> Result<Self, JsValue> {
        if config.arrival_rate_per_min <= 0.0 {
            return Err(JsValue::from_str("arrival rate must be positive"));
        }
        if config.service_rate_per_min <= 0.0 {
            return Err(JsValue::from_str("service rate must be positive"));
        }
        if config.server_count == 0 {
            return Err(JsValue::from_str("server count must be at least 1"));
        }
        if config.queue_capacity == 0 {
            return Err(JsValue::from_str("queue capacity must be at least 1"));
        }

        let mut simulation = Self {
            queue: VecDeque::with_capacity(config.queue_capacity),
            servers: vec![None; config.server_count],
            stats: Stats::new(config.server_count),
            elapsed: 0.0,
            time_since_arrival: 0.0,
            next_arrival_in: 0.0,
            history: VecDeque::with_capacity(HISTORY_MAX_SAMPLES),
            history_timer: 0.0,
            next_customer_id: 1,
            config,
        };
        simulation.next_arrival_in = simulation.sample_next_arrival();
        simulation.push_history_sample();
        Ok(simulation)
    }

    fn config(&self) -> &Config {
        &self.config
    }
    fn update(&mut self, dt: f64) {
        if dt <= 0.0 {
            return;
        }

        self.elapsed += dt;
        self.time_since_arrival += dt;
        self.history_timer += dt;
        self.stats.queue_area += self.queue.len() as f64 * dt;

        while self.time_since_arrival >= self.next_arrival_in {
            self.time_since_arrival -= self.next_arrival_in;
            self.spawn_customer();
            self.next_arrival_in = self.sample_next_arrival();
        }

        for index in 0..self.servers.len() {
            if let Some(mut state) = self.servers[index] {
                state.service_remaining -= dt;
                self.stats.busy_time_by_server[index] += dt.max(0.0);
                if state.service_remaining <= 0.0 {
                    self.servers[index] = None;
                    self.finish_service(index, state);
                } else {
                    self.servers[index] = Some(state);
                }
            } else {
                self.try_start_service(index);
            }
        }

        self.stats.peak_queue = self.stats.peak_queue.max(self.queue.len());

        while self.history_timer >= HISTORY_INTERVAL {
            self.history_timer -= HISTORY_INTERVAL;
            self.push_history_sample();
        }
    }
    fn spawn_customer(&mut self) {
        let customer = Customer {
            id: self.next_customer_id,
            arrival_at: self.elapsed,
        };
        self.next_customer_id = self
            .next_customer_id
            .checked_add(1)
            .expect("customer id overflow");
        self.stats.arrivals += 1;

        if !self.enqueue_customer(customer) {
            self.stats.dropped += 1;
        }
    }

    fn enqueue_customer(&mut self, customer: Customer) -> bool {
        if self.queue.len() >= self.config.queue_capacity {
            false
        } else {
            self.queue.push_back(customer);
            true
        }
    }
    fn try_start_service(&mut self, index: usize) {
        if self.queue.is_empty() {
            return;
        }
        let customer = self
            .queue
            .pop_front()
            .expect("queue unexpectedly empty when starting service");
        let service_time = self.sample_service_time();
        self.servers[index] = Some(ServerState {
            customer,
            service_remaining: service_time,
            service_total: service_time,
            started_at: self.elapsed,
        });
    }

    fn finish_service(&mut self, index: usize, state: ServerState) {
        let wait_time = (state.started_at - state.customer.arrival_at).max(0.0);
        let system_time = (self.elapsed - state.customer.arrival_at).max(0.0);
        self.stats.total_wait_time += wait_time;
        self.stats.total_system_time += system_time;
        self.stats.served += 1;
        self.try_start_service(index);
    }

    fn push_history_sample(&mut self) {
        let in_service = self.servers.iter().filter(|slot| slot.is_some()).count();
        let sample = HistorySample {
            time: self.elapsed,
            queue_len: self.queue.len(),
            in_system: self.queue.len() + in_service,
        };
        if self.history.len() == HISTORY_MAX_SAMPLES {
            self.history.pop_front();
        }
        self.history.push_back(sample);
    }
    fn sample_next_arrival(&self) -> f64 {
        let rate = self.config.arrival_rate_per_sec();
        match self.config.arrival_pattern {
            ArrivalPattern::Deterministic => (1.0 / rate).max(0.01),
            ArrivalPattern::Poisson => sample_exponential(rate),
            ArrivalPattern::Bursty => {
                let draw = random_unit();
                if draw < 0.3 {
                    sample_exponential(rate * 2.4).max(0.02)
                } else {
                    (sample_exponential(rate * 0.6) + random_unit() * 0.8).max(0.02)
                }
            }
        }
    }

    fn sample_service_time(&self) -> f64 {
        let rate = self.config.service_rate_per_sec();
        match self.config.service_pattern {
            ServicePattern::Deterministic => (1.0 / rate).max(0.01),
            ServicePattern::Exponential => sample_exponential(rate),
            ServicePattern::Erlang => sample_erlang(rate, 2),
        }
    }
    fn rho(&self) -> f64 {
        self.config.arrival_rate_per_min
            / (self.config.service_rate_per_min * self.config.server_count as f64)
    }

    fn utilization(&self) -> f64 {
        if self.elapsed <= 0.0 {
            return 0.0;
        }
        let total_busy: f64 = self.stats.busy_time_by_server.iter().copied().sum();
        total_busy / (self.elapsed * self.config.server_count as f64)
    }

    fn throughput_per_min(&self) -> f64 {
        if self.elapsed <= 0.0 {
            0.0
        } else {
            self.stats.served as f64 / self.elapsed * 60.0
        }
    }

    fn effective_arrival_per_min(&self) -> f64 {
        if self.elapsed <= 0.0 {
            0.0
        } else {
            self.stats.arrivals as f64 / self.elapsed * 60.0
        }
    }

    fn average_wait_time(&self) -> f64 {
        if self.stats.served == 0 {
            0.0
        } else {
            self.stats.total_wait_time / self.stats.served as f64
        }
    }

    fn average_system_time(&self) -> f64 {
        if self.stats.served == 0 {
            0.0
        } else {
            self.stats.total_system_time / self.stats.served as f64
        }
    }

    fn average_queue_length(&self) -> f64 {
        if self.elapsed <= 0.0 {
            self.queue.len() as f64
        } else {
            self.stats.queue_area / self.elapsed
        }
    }

    fn loss_percentage(&self) -> f64 {
        if self.stats.arrivals == 0 {
            0.0
        } else {
            (self.stats.dropped as f64 / self.stats.arrivals as f64) * 100.0
        }
    }

    fn history(&self) -> &VecDeque<HistorySample> {
        &self.history
    }

    fn queue(&self) -> &VecDeque<Customer> {
        &self.queue
    }

    fn servers(&self) -> &[Option<ServerState>] {
        &self.servers
    }

    fn stats(&self) -> &Stats {
        &self.stats
    }
}
impl Simulation {
    fn set_arrival_rate(&mut self, per_minute: f64) -> Result<(), JsValue> {
        if per_minute <= 0.0 {
            return Err(JsValue::from_str("arrival rate must be positive"));
        }
        self.config.arrival_rate_per_min = per_minute;
        self.time_since_arrival = 0.0;
        self.next_arrival_in = self.sample_next_arrival();
        Ok(())
    }

    fn set_service_rate(&mut self, per_minute: f64) -> Result<(), JsValue> {
        if per_minute <= 0.0 {
            return Err(JsValue::from_str("service rate must be positive"));
        }
        self.config.service_rate_per_min = per_minute;
        Ok(())
    }

    fn set_server_count(&mut self, count: usize) -> Result<(), JsValue> {
        if count == 0 {
            return Err(JsValue::from_str("server count must be at least one"));
        }
        if count == self.config.server_count {
            return Ok(());
        }

        let mut active_states: Vec<ServerState> = self
            .servers
            .iter_mut()
            .filter_map(|slot| slot.take())
            .collect();
        active_states.sort_by(|a, b| {
            a.started_at
                .partial_cmp(&b.started_at)
                .expect("NaN in started_at")
        });

        self.servers = vec![None; count];
        self.stats.resize_servers(count);
        self.config.server_count = count;

        let mut requeue: Vec<Customer> = Vec::new();
        for state in active_states {
            if let Some(slot) = self.servers.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(ServerState {
                    customer: state.customer,
                    service_remaining: state.service_remaining,
                    service_total: state.service_total,
                    started_at: state.started_at,
                });
            } else {
                requeue.push(state.customer);
            }
        }

        if !requeue.is_empty() {
            for customer in requeue.iter().rev() {
                if !self.enqueue_customer(*customer) {
                    self.stats.dropped += 1;
                }
            }
        }

        Ok(())
    }

    fn set_queue_capacity(&mut self, capacity: usize) -> Result<(), JsValue> {
        if capacity == 0 {
            return Err(JsValue::from_str("queue capacity must be at least one"));
        }
        self.config.queue_capacity = capacity;
        while self.queue.len() > capacity {
            let _ = self.queue.pop_back();
            self.stats.dropped += 1;
        }
        Ok(())
    }

    fn set_arrival_pattern(&mut self, pattern: ArrivalPattern) -> Result<(), JsValue> {
        self.config.arrival_pattern = pattern;
        self.time_since_arrival = 0.0;
        self.next_arrival_in = self.sample_next_arrival();
        Ok(())
    }

    fn set_service_pattern(&mut self, pattern: ServicePattern) -> Result<(), JsValue> {
        self.config.service_pattern = pattern;
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
fn random_unit() -> f64 {
    js_sys::Math::random()
}

#[cfg(not(target_arch = "wasm32"))]
fn random_unit() -> f64 {
    static SEED: AtomicU64 = AtomicU64::new(0x0123_4567_89ab_cdef);
    let current = SEED.load(Ordering::Relaxed);
    let next = current
        .wrapping_mul(636_413_622_384_679_3005)
        .wrapping_add(1);
    SEED.store(next, Ordering::Relaxed);
    let bits = (next >> 11) | 1;
    (bits as f64) / ((1u64 << 53) as f64)
}

fn sample_exponential(rate: f64) -> f64 {
    if rate <= 0.0 {
        panic!("exponential distribution requires positive rate");
    }
    let mut u = random_unit();
    if u <= f64::EPSILON {
        u = f64::EPSILON;
    }
    -u.ln() / rate
}

fn sample_erlang(rate: f64, shape: u32) -> f64 {
    if shape == 0 {
        panic!("erlang shape must be positive");
    }
    let per_stage_rate = rate * shape as f64;
    let mut total = 0.0;
    for _ in 0..shape {
        total += sample_exponential(per_stage_rate);
    }
    total
}
struct Renderer {
    width: f64,
    height: f64,
}

impl Renderer {
    fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    fn draw(&self, context: &CanvasRenderingContext2d, simulation: &Simulation) {
        context.set_fill_style_str(CANVAS_BG);
        context.fill_rect(0.0, 0.0, self.width, self.height);

        self.draw_queue_area(context, simulation);
        self.draw_server_area(context, simulation);
        self.draw_metrics(context, simulation);
        self.draw_history(context, simulation);
    }

    fn draw_queue_area(&self, context: &CanvasRenderingContext2d, simulation: &Simulation) {
        let base_x = 40.0;
        let base_y = self.height - 160.0;
        let spacing = 28.0;
        let radius = 10.0;

        for (index, customer) in simulation.queue().iter().enumerate() {
            let x = base_x + spacing * index as f64;
            let y = base_y - (index % 2) as f64 * 18.0;
            context.set_fill_style_str(if index == 0 {
                WAITING_COLOR
            } else {
                CUSTOMER_COLOR
            });
            context.begin_path();
            context
                .arc(x, y, radius, 0.0, std::f64::consts::TAU)
                .expect("failed to draw queue arc");
            context.fill();
            context.set_fill_style_str(TEXT_SECONDARY);
            context.set_font("10px 'Segoe UI', sans-serif");
            context
                .fill_text(&format!("#{:02}", customer.id % 100), x - 12.0, y + 20.0)
                .expect("failed to draw queue id");
        }

        context.set_stroke_style_str(TEXT_SECONDARY);
        context.begin_path();
        context.move_to(base_x - 20.0, base_y + 32.0);
        context.line_to(
            base_x + spacing * (simulation.config.queue_capacity as f64 + 0.5),
            base_y + 32.0,
        );
        context.stroke();
    }

    fn draw_server_area(&self, context: &CanvasRenderingContext2d, simulation: &Simulation) {
        let area_x = 40.0;
        let area_y = 80.0;
        let slot_width = 120.0;
        let slot_height = 80.0;
        let gap = 18.0;

        for (index, slot) in simulation.servers().iter().enumerate() {
            let x = area_x + (slot_width + gap) * index as f64;
            let y = area_y;
            context.set_fill_style_str(if slot.is_some() {
                SERVER_BUSY_COLOR
            } else {
                SERVER_IDLE_COLOR
            });
            context.fill_rect(x, y, slot_width, slot_height);
            context.set_fill_style_str(TEXT_PRIMARY);
            context.set_font("14px 'Segoe UI', sans-serif");
            context
                .fill_text(&format!("窓口 {}", index + 1), x + 12.0, y + 24.0)
                .expect("failed to draw server title");

            if let Some(state) = slot {
                let progress = if state.service_total <= 0.0 {
                    1.0
                } else {
                    1.0 - (state.service_remaining / state.service_total).clamp(0.0, 1.0)
                };
                let bar_width = (slot_width - 24.0) * progress;
                context.set_fill_style_str("#0f172a");
                context.fill_rect(x + 12.0, y + 36.0, slot_width - 24.0, 18.0);
                context.set_fill_style_str("#fde68a");
                context.fill_rect(x + 12.0, y + 36.0, bar_width, 18.0);
                context.set_fill_style_str(TEXT_PRIMARY);
                context.set_font("12px 'Segoe UI', sans-serif");
                context
                    .fill_text(
                        &format!("進捗 {:>3}%", (progress * 100.0).round() as i32),
                        x + 16.0,
                        y + 52.0,
                    )
                    .expect("failed to draw progress text");
            } else {
                context.set_fill_style_str(TEXT_SECONDARY);
                context.set_font("12px 'Segoe UI', sans-serif");
                context
                    .fill_text("待機中", x + 16.0, y + 52.0)
                    .expect("failed to draw idle text");
            }
        }
    }

    fn draw_metrics(&self, context: &CanvasRenderingContext2d, simulation: &Simulation) {
        context.set_fill_style_str(TEXT_PRIMARY);
        context.set_font("16px 'Segoe UI', sans-serif");
        context
            .fill_text("主要指標", self.width - 280.0, 90.0)
            .expect("failed to draw metrics title");

        let rho = simulation.rho();
        let utilization = simulation.utilization();
        let throughput = simulation.throughput_per_min();
        let lambda_eff = simulation.effective_arrival_per_min();
        let avg_wait = simulation.average_wait_time();
        let avg_system = simulation.average_system_time();
        let avg_queue = simulation.average_queue_length();
        let loss = simulation.loss_percentage();

        let lines = [
            format!("ρ (負荷率): {:.2}", rho),
            format!("稼働率: {:.1}%", utilization * 100.0),
            format!("処理スループット: {:.1} 人/分", throughput),
            format!("実到着率: {:.1} 人/分", lambda_eff),
            format!("平均待ち時間 Wq: {:.1} 秒", avg_wait),
            format!("平均滞在時間 W: {:.1} 秒", avg_system),
            format!("平均待ち人数 Lq: {:.1} 人", avg_queue),
            format!("離脱率: {:.1}%", loss),
            format!("累計到着: {} 人", simulation.stats().arrivals),
            format!("累計処理: {} 人", simulation.stats().served),
            format!("累計離脱: {} 人", simulation.stats().dropped),
        ];

        context.set_font("13px 'Segoe UI', sans-serif");
        for (i, line) in lines.iter().enumerate() {
            context
                .fill_text(line, self.width - 280.0, 120.0 + 22.0 * i as f64)
                .expect("failed to draw metric line");
        }

        context.set_fill_style_str(if rho >= 1.0 {
            DROPPED_COLOR
        } else {
            TEXT_SECONDARY
        });
        context.set_font("12px 'Segoe UI', sans-serif");
        context
            .fill_text(
                if rho >= 1.0 {
                    "⚠ ρ が 1 を超えました。待ち時間が急増しています。"
                } else {
                    "ρ < 1 を保つと安定稼働が期待できます。"
                },
                self.width - 280.0,
                370.0,
            )
            .expect("failed to draw rho warning");
    }

    fn draw_history(&self, context: &CanvasRenderingContext2d, simulation: &Simulation) {
        let area_x = self.width - 320.0;
        let area_y = self.height - 180.0;
        let area_width = 260.0;
        let area_height = 120.0;

        context.set_fill_style_str("#111827");
        context.fill_rect(area_x, area_y, area_width, area_height);
        context.set_stroke_style_str("#1f2937");
        context.stroke_rect(area_x, area_y, area_width, area_height);

        context.set_stroke_style_str("#334155");
        context.begin_path();
        context.move_to(area_x, area_y + area_height - 1.0);
        context.line_to(area_x + area_width, area_y + area_height - 1.0);
        context.stroke();

        let history = simulation.history();
        if history.len() < 2 {
            return;
        }

        let latest_time = history
            .back()
            .expect("history should contain last sample")
            .time;
        let earliest_time = history
            .front()
            .expect("history should contain first sample")
            .time;
        let duration = (latest_time - earliest_time).max(1.0);
        let queue_max = simulation
            .stats()
            .peak_queue
            .max(simulation.config.queue_capacity);
        let scale_y = if queue_max == 0 {
            1.0
        } else {
            queue_max as f64
        };

        context.set_stroke_style_str("#38bdf8");
        context.begin_path();
        for (index, sample) in history.iter().enumerate() {
            let progress = (sample.time - earliest_time) / duration;
            let x = area_x + progress * area_width;
            let y =
                area_y + area_height - (sample.queue_len as f64 / scale_y) * (area_height - 10.0);
            if index == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.stroke();

        context.set_stroke_style_str("#a855f7");
        context.begin_path();
        for (index, sample) in history.iter().enumerate() {
            let progress = (sample.time - earliest_time) / duration;
            let x = area_x + progress * area_width;
            let y =
                area_y + area_height - (sample.in_system as f64 / scale_y) * (area_height - 10.0);
            if index == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.stroke();

        context.set_fill_style_str(TEXT_PRIMARY);
        context.set_font("12px 'Segoe UI', sans-serif");
        context
            .fill_text(
                "人数推移 (青: 待機 / 紫: 系内)",
                area_x + 12.0,
                area_y + 16.0,
            )
            .expect("failed to draw history label");
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn make_simulation() -> Simulation {
        Simulation::new(720.0, 480.0, Config::default())
            .expect("failed to construct simulation for tests")
    }

    #[test]
    fn queue_capacity_drops_excess_customers() {
        let mut sim = make_simulation();
        sim.set_queue_capacity(2)
            .expect("queue capacity update should succeed");

        for _ in 0..5 {
            sim.spawn_customer();
        }

        assert_eq!(sim.stats().arrivals, 5);
        assert_eq!(sim.stats().dropped, 3);
        assert_eq!(sim.queue().len(), 2);
    }

    #[test]
    fn reducing_server_count_requeues_busy_customers() {
        let mut sim = make_simulation();
        sim.set_server_count(3)
            .expect("initial server count update should succeed");

        for index in 0..3 {
            sim.spawn_customer();
            sim.try_start_service(index);
        }

        assert_eq!(
            sim.servers().iter().filter(|slot| slot.is_some()).count(),
            3
        );

        sim.set_server_count(1)
            .expect("reducing server count should succeed");

        assert_eq!(
            sim.servers().iter().filter(|slot| slot.is_some()).count(),
            1
        );
        assert!(
            sim.queue().len() >= 2,
            "remaining customers should be re-queued"
        );
        assert_eq!(
            sim.stats().dropped,
            0,
            "no customers should be dropped during reconfiguration"
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn invalid_parameters_are_rejected() {
        let mut sim = make_simulation();
        assert!(sim.set_arrival_rate(0.0).is_err());
        assert!(sim.set_service_rate(0.0).is_err());
        assert!(sim.set_server_count(0).is_err());
        assert!(sim.set_queue_capacity(0).is_err());
    }
}
