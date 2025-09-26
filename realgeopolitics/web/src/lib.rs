#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use realgeopolitics_core::CountryDefinition;

#[cfg(target_arch = "wasm32")]
use realgeopolitics_core::{BudgetAllocation, GameState, TimeStatus};
use serde_json::Error as SerdeError;

#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Interval;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlInputElement, HtmlSelectElement};
#[cfg(target_arch = "wasm32")]
use yew::prelude::*;

const DEFAULT_COUNTRIES: &str = r#"[
    {
        "name": "Asteria",
        "government": "Parliamentary Republic",
        "population_millions": 62.5,
        "gdp": 2100.0,
        "stability": 64,
        "military": 55,
        "approval": 52,
        "budget": 620.0,
        "resources": 75
    },
    {
        "name": "Borealis Union",
        "government": "Federal Technocracy",
        "population_millions": 48.3,
        "gdp": 1780.0,
        "stability": 71,
        "military": 68,
        "approval": 47,
        "budget": 540.0,
        "resources": 92
    },
    {
        "name": "Caldoria",
        "government": "Constitutional Monarchy",
        "population_millions": 35.9,
        "gdp": 1330.0,
        "stability": 58,
        "military": 61,
        "approval": 60,
        "budget": 470.0,
        "resources": 64
    }
]"#;

fn load_default_definitions() -> Result<Vec<CountryDefinition>, SerdeError> {
    serde_json::from_str::<Vec<CountryDefinition>>(DEFAULT_COUNTRIES)
}

#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
struct SpeedOption {
    value: String,
    label: String,
}

#[cfg(any(test, target_arch = "wasm32"))]
fn build_speed_options(speed_value: f64, presets: &[(f64, &str)]) -> Vec<SpeedOption> {
    let mut options = presets
        .iter()
        .map(|(value, label)| SpeedOption {
            value: format!("{:.2}", value),
            label: format!("{} (x{:.2})", label, value),
        })
        .collect::<Vec<_>>();
    if presets
        .iter()
        .all(|(value, _)| (value - speed_value).abs() > 0.01)
    {
        options.push(SpeedOption {
            value: format!("{:.2}", speed_value),
            label: format!("カスタム (x{:.2})", speed_value),
        });
    }
    options
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, PartialEq)]
struct AllocationForm {
    infrastructure: f64,
    military: f64,
    welfare: f64,
    diplomacy: f64,
}

#[cfg(target_arch = "wasm32")]
impl AllocationForm {
    fn from_allocation(allocation: BudgetAllocation) -> Self {
        Self {
            infrastructure: allocation.infrastructure * 100.0,
            military: allocation.military * 100.0,
            welfare: allocation.welfare * 100.0,
            diplomacy: allocation.diplomacy * 100.0,
        }
    }

    fn update(mut self, field: AllocationField, value: f64) -> Self {
        let clamped = value.clamp(0.0, 100.0);
        match field {
            AllocationField::Infrastructure => self.infrastructure = clamped,
            AllocationField::Military => self.military = clamped,
            AllocationField::Welfare => self.welfare = clamped,
            AllocationField::Diplomacy => self.diplomacy = clamped,
        }
        self.normalize()
    }

    fn normalize(mut self) -> Self {
        let total = self.total();
        if total > 100.0 + f64::EPSILON {
            let factor = 100.0 / total;
            self.infrastructure *= factor;
            self.military *= factor;
            self.welfare *= factor;
            self.diplomacy *= factor;
        }
        self
    }

    fn total(&self) -> f64 {
        self.infrastructure + self.military + self.welfare + self.diplomacy
    }

    fn to_budget_allocation(&self) -> Result<BudgetAllocation, String> {
        BudgetAllocation::from_percentages(
            self.infrastructure,
            self.military,
            self.welfare,
            self.diplomacy,
        )
        .map_err(|err| err.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum AllocationField {
    Infrastructure,
    Military,
    Welfare,
    Diplomacy,
}

#[cfg(target_arch = "wasm32")]
#[function_component(App)]
fn app() -> Html {
    let initial_definitions = load_default_definitions().expect("国データの読み込みに失敗しました");
    let game = use_mut_ref(|| {
        GameState::from_definitions(initial_definitions).expect("国データの初期化に失敗しました")
    });

    let initial_forms = {
        let game_ref = game.borrow();
        game_ref
            .countries()
            .iter()
            .map(|country| AllocationForm::from_allocation(country.allocations()))
            .collect::<Vec<_>>()
    };

    let allocation_forms = use_state(|| initial_forms);
    let selected_country = use_state(|| 0usize);
    let message = use_state(|| Option::<String>::None);
    let reports = use_state(Vec::<String>::new);
    let refresh = use_state(|| 0u32);

    {
        let game = game.clone();
        let forms_handle = allocation_forms.clone();
        let reports_handle = reports.clone();
        let message_handle = message.clone();
        let refresh = refresh.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(1000, move || {
                let mut game_mut = game.borrow_mut();
                match game_mut.tick_minutes(10.0) {
                    Ok(new_reports) => {
                        if !new_reports.is_empty() {
                            let mut aggregated = (*reports_handle).clone();
                            aggregated.extend(new_reports.into_iter());
                            if aggregated.len() > 12 {
                                let len = aggregated.len();
                                aggregated = aggregated[len - 12..].to_vec();
                            }
                            reports_handle.set(aggregated);
                        }
                        let snapshot = game_mut
                            .countries()
                            .iter()
                            .map(|country| AllocationForm::from_allocation(country.allocations()))
                            .collect::<Vec<_>>();
                        forms_handle.set(snapshot);
                        refresh.set(refresh.wrapping_add(1));
                    }
                    Err(err) => {
                        message_handle.set(Some(err.to_string()));
                    }
                }
            });
            move || drop(interval)
        });
    }

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |event: Event| {
            if let Some(value) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlSelectElement>().ok())
            {
                if let Ok(idx) = value.value().parse::<usize>() {
                    selected_country.set(idx);
                }
            }
        })
    };

    let on_speed_change = {
        let game = game.clone();
        let message = message.clone();
        Callback::from(move |event: Event| {
            if let Some(select) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlSelectElement>().ok())
            {
                match select.value().parse::<f64>() {
                    Ok(multiplier) => {
                        if let Err(err) = game.borrow_mut().set_time_multiplier(multiplier) {
                            message.set(Some(err.to_string()));
                        } else {
                            message.set(None);
                        }
                    }
                    Err(_) => {
                        message.set(Some("時間倍率は数値で指定してください。".to_string()));
                    }
                }
            }
        })
    };

    let update_slider = {
        let game = game.clone();
        let forms = allocation_forms.clone();
        let message = message.clone();
        Callback::from(move |payload: SliderChange| {
            let mut current_forms = (*forms).clone();
            if payload.country_idx >= current_forms.len() {
                return;
            }
            let updated = current_forms[payload.country_idx].update(payload.field, payload.value);
            match updated.to_budget_allocation() {
                Ok(allocation) => {
                    if let Err(err) = game
                        .borrow_mut()
                        .update_allocations(payload.country_idx, allocation)
                    {
                        message.set(Some(err.to_string()));
                        return;
                    }
                    current_forms[payload.country_idx] = updated;
                    forms.set(current_forms);
                    message.set(None);
                }
                Err(err) => {
                    message.set(Some(err));
                }
            }
        })
    };

    let countries_snapshot = game.borrow();
    let status: TimeStatus = countries_snapshot.time_status();
    let countries = countries_snapshot.countries();
    let sim_minutes = status.simulation_minutes;
    let calendar = status.calendar;
    let next_event = status
        .next_event_in_minutes
        .map(|m| format!("{:.1} 分", m as f64))
        .unwrap_or_else(|| "未定".to_string());
    let speed_value = status.time_multiplier;
    let speed_value_str = format!("{:.2}", speed_value);
    let speed_presets: &[(f64, &str)] =
        &[(0.5, "低速"), (1.0, "標準"), (2.0, "高速"), (4.0, "超高速")];
    let commodity_price = countries_snapshot.commodity_price();
    let speed_options = build_speed_options(speed_value, speed_presets);
    let current_idx = (*selected_country).min(countries.len().saturating_sub(1));
    let current_allocation = allocation_forms
        .get(current_idx)
        .copied()
        .unwrap_or(AllocationForm {
            infrastructure: 0.0,
            military: 0.0,
            welfare: 0.0,
            diplomacy: 0.0,
        });
    let remaining = (100.0 - current_allocation.total()).max(0.0);

    let relations: Vec<(String, i32)> = countries
        .get(current_idx)
        .map(|country| {
            let mut pairs: Vec<_> = country
                .relations
                .iter()
                .map(|(name, value)| (name.clone(), *value))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            pairs
        })
        .unwrap_or_default();

    let reports_view = (*reports)
        .iter()
        .enumerate()
        .map(|(idx, report)| html! { <li key={idx}>{ report }</li> })
        .collect::<Html>();

    let message_view = if let Some(msg) = &*message {
        html! { <div class="message error">{ msg }</div> }
    } else {
        Html::default()
    };

    let speed_option_nodes: Vec<Html> = speed_options
        .iter()
        .enumerate()
        .map(|(idx, option)| {
            html! {
                <option key={idx.to_string()} value={option.value.clone()}>{ option.label.clone() }</option>
            }
        })
        .collect();

    html! {
        <div class="app" data-refresh={(*refresh).to_string()}>
            <header>
                <div class="time-panel">
                    <h1>{ "リアル・ジオポリティクス シミュレーター" }</h1>
                    <p>{ format!("シミュレーション時間 {:.1} 分 (日付 {:04}-{:02}-{:02})", sim_minutes, calendar.year, calendar.month, calendar.day) }</p>
                    <p>{ format!("次イベントまで: {}", next_event) }</p>
                </div>
                <div class="summary">
                    <span>{ "監視中の国家数: " }{ countries.len() }</span>
                    <span>{ format!("時間倍率: x{:.2}", speed_value) }</span>
                    <span>{ format!("資源価格: {:.1}", commodity_price) }</span>
                    <label class="speed-control">
                        { "速度" }
                        <select onchange={on_speed_change.clone()} value={speed_value_str.clone()}>
                            { for speed_option_nodes.iter().cloned() }
                        </select>
                    </label>
                </div>
            </header>

            { message_view }

            <section class="controls">
                <label>
                    { "対象国" }
                    <select onchange={on_country_change.clone()} value={current_idx.to_string()}>
                        { for countries.iter().enumerate().map(|(idx, country)| {
                            html! { <option value={idx.to_string()}>{ format!("{}: {}", idx + 1, country.name) }</option> }
                        }) }
                    </select>
                </label>
                <div class="allocation-summary">
                    <span>{ format!("配分合計: {:.1}%", current_allocation.total()) }</span>
                    <span>{ format!("未割当: {:.1}%", remaining) }</span>
                </div>
            </section>

            <section class="sliders">
                <h2>{ "予算配分" }</h2>
                { render_slider("インフラ", current_allocation.infrastructure, current_idx, AllocationField::Infrastructure, update_slider.clone()) }
                { render_slider("軍事", current_allocation.military, current_idx, AllocationField::Military, update_slider.clone()) }
                { render_slider("福祉", current_allocation.welfare, current_idx, AllocationField::Welfare, update_slider.clone()) }
                { render_slider("外交", current_allocation.diplomacy, current_idx, AllocationField::Diplomacy, update_slider.clone()) }
            </section>

            <section class="overview">
                <h2>{ "主要指標" }</h2>
                <table>
                    <thead>
                        <tr>
                            <th>{ "ID" }</th>
                            <th>{ "国名" }</th>
                            <th>{ "政体" }</th>
                            <th>{ "GDP" }</th>
                            <th>{ "安定" }</th>
                            <th>{ "軍事" }</th>
                            <th>{ "支持" }</th>
                            <th>{ "予算" }</th>
                            <th>{ "収入" }</th>
                            <th>{ "支出" }</th>
                            <th>{ "資源" }</th>
                        </tr>
                    </thead>
                    <tbody>
                        { for countries.iter().enumerate().map(|(idx, country)| {
                            let row_class = if idx == current_idx { "selected" } else { "" };
                            html! {
                                <tr class={row_class}>
                                    <td>{ idx + 1 }</td>
                                    <td>{ &country.name }</td>
                                    <td>{ &country.government }</td>
                                    <td>{ format!("{:.1}", country.gdp) }</td>
                                    <td>{ country.stability }</td>
                                    <td>{ country.military }</td>
                                    <td>{ country.approval }</td>
                                    <td>{ format!("{:.1}", country.cash_reserve()) }</td>
                                    <td>{ format!("{:.1}", country.total_revenue()) }</td>
                                    <td>{ format!("{:.1}", country.total_expense()) }</td>
                                    <td>{ country.resources }</td>
                                </tr>
                            }
                        }) }
                    </tbody>
                </table>
            </section>

            <section class="relations">
                <h2>{ format!("{} の外交関係", countries.get(current_idx).map(|c| c.name.as_str()).unwrap_or("-")) }</h2>
                <ul>
                    { for relations.iter().map(|(partner, value)| {
                        html! { <li key={partner.clone()}>{ format!("{}: {}", partner, value) }</li> }
                    }) }
                </ul>
            </section>

            <section class="reports">
                <h2>{ "最新イベント" }</h2>
                <ul>{ reports_view }</ul>
            </section>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct SliderChange {
    country_idx: usize,
    field: AllocationField,
    value: f64,
}

#[cfg(target_arch = "wasm32")]
fn render_slider(
    label: &str,
    value: f64,
    country_idx: usize,
    field: AllocationField,
    callback: Callback<SliderChange>,
) -> Html {
    let onchange = {
        let callback = callback.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            {
                if let Ok(value) = input.value().parse::<f64>() {
                    callback.emit(SliderChange {
                        country_idx,
                        field,
                        value,
                    });
                }
            }
        })
    };

    html! {
        <div class="slider-row">
            <label>{ format!("{}: {:.1}%", label, value) }</label>
            <input type="range" min="0" max="100" step="1" value={format!("{:.0}", value)} oninput={onchange} />
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    yew::Renderer::<App>::new().render();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start() {
    panic!("realgeopolitics-web は wasm32-unknown-unknown ターゲットでのみ利用できます。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_speed_options_appends_custom_when_needed() {
        let presets = [(0.5, "低速"), (1.0, "標準"), (2.0, "高速")];
        let options = build_speed_options(1.7, &presets);
        assert!(options.iter().any(|opt| opt.label.contains("カスタム")));
        assert_eq!(options.last().map(|opt| opt.value.as_str()), Some("1.70"));
    }

    #[test]
    fn build_speed_options_omits_custom_when_matching() {
        let presets = [(0.5, "低速"), (1.0, "標準"), (2.0, "高速")];
        let options = build_speed_options(1.0, &presets);
        assert!(!options.iter().any(|opt| opt.label.contains("カスタム")));
        assert_eq!(options.len(), presets.len());
    }
}
