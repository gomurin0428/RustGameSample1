#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use realgeopolitics_core::CountryDefinition;

#[cfg(target_arch = "wasm32")]
use realgeopolitics_core::{
    BudgetAllocation, FiscalSnapshot, FiscalTrendPoint, GameState, TimeStatus,
};
use serde_json::Error as SerdeError;

#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Interval;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::{Event, HtmlInputElement, HtmlSelectElement, InputEvent};
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
    debt_service: f64,
    administration: f64,
    research: f64,
    ensure_core_minimum: bool,
}

#[cfg(target_arch = "wasm32")]
impl AllocationForm {
    fn from_allocation(allocation: BudgetAllocation) -> Self {
        Self {
            infrastructure: allocation.infrastructure,
            military: allocation.military,
            welfare: allocation.welfare,
            diplomacy: allocation.diplomacy,
            debt_service: allocation.debt_service,
            administration: allocation.administration,
            research: allocation.research,
            ensure_core_minimum: allocation.ensure_core_minimum,
        }
    }

    fn update_amount(mut self, field: AllocationField, value: f64) -> Self {
        let clamped = value.max(0.0);
        match field {
            AllocationField::Infrastructure => self.infrastructure = clamped,
            AllocationField::Military => self.military = clamped,
            AllocationField::Welfare => self.welfare = clamped,
            AllocationField::Diplomacy => self.diplomacy = clamped,
            AllocationField::DebtService => self.debt_service = clamped,
            AllocationField::Administration => self.administration = clamped,
            AllocationField::Research => self.research = clamped,
        }
        self
    }

    fn set_core_minimum(mut self, enabled: bool) -> Self {
        self.ensure_core_minimum = enabled;
        self
    }

    fn total(&self) -> f64 {
        self.infrastructure
            + self.military
            + self.welfare
            + self.diplomacy
            + self.debt_service
            + self.administration
            + self.research
    }

    fn to_budget_allocation(&self) -> Result<BudgetAllocation, String> {
        BudgetAllocation::new(
            self.infrastructure,
            self.military,
            self.welfare,
            self.diplomacy,
            self.debt_service,
            self.administration,
            self.research,
            self.ensure_core_minimum,
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
    DebtService,
    Administration,
    Research,
}

#[cfg(target_arch = "wasm32")]
#[function_component(App)]
fn app() -> Html {
    let initial_definitions = load_default_definitions().expect("国データの読み込みに失敗しました");
    let game = use_mut_ref(|| {
        GameState::from_definitions(initial_definitions).expect("国データの初期化に失敗しました")
    });

    let (initial_forms, initial_snapshots) = {
        let game_ref = game.borrow();
        let forms = game_ref
            .countries()
            .iter()
            .map(|country| AllocationForm::from_allocation(country.allocations()))
            .collect::<Vec<_>>();
        let snapshots = game_ref.fiscal_snapshots();
        (forms, snapshots)
    };

    let allocation_forms = use_state(|| initial_forms);
    let fiscal_snapshots = use_state(|| initial_snapshots);
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
        let snapshots_handle = fiscal_snapshots.clone();
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
                        let fiscal_view = game_mut.fiscal_snapshots();
                        snapshots_handle.set(fiscal_view);
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

    let update_amount = {
        let game = game.clone();
        let forms = allocation_forms.clone();
        let message = message.clone();
        Callback::from(move |payload: AllocationAmountChange| {
            let mut current_forms = (*forms).clone();
            if payload.country_idx >= current_forms.len() {
                return;
            }
            let updated =
                current_forms[payload.country_idx].update_amount(payload.field, payload.value);
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

    let update_core = {
        let game = game.clone();
        let forms = allocation_forms.clone();
        let message = message.clone();
        Callback::from(move |payload: CoreMinimumChange| {
            let mut current_forms = (*forms).clone();
            if payload.country_idx >= current_forms.len() {
                return;
            }
            let updated = current_forms[payload.country_idx].set_core_minimum(payload.enabled);
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
            debt_service: 0.0,
            administration: 0.0,
            research: 0.0,
            ensure_core_minimum: true,
        });
    let total_budget = current_allocation.total();

    let current_country = countries.get(current_idx);

    let snapshots_ref: &Vec<FiscalSnapshot> = &*fiscal_snapshots;
    let current_snapshot = snapshots_ref
        .get(current_idx)
        .unwrap_or_else(|| panic!("財政スナップショットが存在しません (idx: {})", current_idx));

    let gdp_value = current_country.map(|country| country.gdp).unwrap_or(0.0);
    let cash_value = current_snapshot.cash_reserve;
    let debt_value = current_snapshot.debt;
    let debt_ratio_value = current_snapshot.debt_ratio;
    let approval_value = current_country
        .map(|country| country.approval as f64)
        .unwrap_or(0.0);
    let dashboard_trend: Vec<FiscalTrendPoint> = {
        let history = &current_snapshot.history;
        let slice_start = history.len().saturating_sub(12);
        history[slice_start..].to_vec()
    };

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
                    <span>{ format!("配分合計: {:.1}%", total_budget) }</span>
                    { render_core_toggle(current_allocation.ensure_core_minimum, current_idx, update_core.clone()) }
                </div>
            </section>

            <section class="allocations">
                <h2>{ "予算配分 (GDP比率 %)" }</h2>
                { render_amount_input("インフラ", current_allocation.infrastructure, current_idx, AllocationField::Infrastructure, update_amount.clone()) }
                { render_amount_input("軍事", current_allocation.military, current_idx, AllocationField::Military, update_amount.clone()) }
                { render_amount_input("福祉", current_allocation.welfare, current_idx, AllocationField::Welfare, update_amount.clone()) }
                { render_amount_input("外交", current_allocation.diplomacy, current_idx, AllocationField::Diplomacy, update_amount.clone()) }
                { render_amount_input("債務返済", current_allocation.debt_service, current_idx, AllocationField::DebtService, update_amount.clone()) }
                { render_amount_input("行政維持", current_allocation.administration, current_idx, AllocationField::Administration, update_amount.clone()) }
                { render_amount_input("研究開発", current_allocation.research, current_idx, AllocationField::Research, update_amount.clone()) }
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
                            <th>{ "予算残高" }</th>
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

            { render_dashboard(
                gdp_value,
                cash_value,
                debt_value,
                approval_value,
                debt_ratio_value,
                &dashboard_trend,
            ) }

            <section class="fiscal-report">
                <h2>{ format!("財政レポート ({})", current_snapshot.name) }</h2>
                { render_fiscal_chart(current_snapshot) }
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
struct AllocationAmountChange {
    country_idx: usize,
    field: AllocationField,
    value: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct CoreMinimumChange {
    country_idx: usize,
    enabled: bool,
}

#[cfg(target_arch = "wasm32")]
fn render_fiscal_chart(snapshot: &FiscalSnapshot) -> Html {
    if snapshot.history.is_empty() {
        panic!("財政履歴が取得できませんでした");
    }
    let max_points = 12;
    let slice_start = snapshot.history.len().saturating_sub(max_points);
    let history_window = &snapshot.history[slice_start..];
    if history_window.is_empty() {
        panic!("財政履歴のウィンドウ生成に失敗しました");
    }
    let width = 560.0;
    let height = 220.0;
    let left_margin = 48.0;
    let right_margin = 16.0;
    let top_margin = 16.0;
    let bottom_margin = 28.0;
    let plot_width = width - left_margin - right_margin;
    let plot_height = height - top_margin - bottom_margin;
    if plot_width <= 0.0 || plot_height <= 0.0 {
        panic!("チャート領域が無効です");
    }

    let min_time = history_window
        .first()
        .map(|point| point.simulation_minutes)
        .expect("履歴開始時間の取得に失敗しました");
    let max_time = history_window
        .last()
        .map(|point| point.simulation_minutes)
        .expect("履歴終了時間の取得に失敗しました");
    let mut time_span = max_time - min_time;
    if !time_span.is_finite() {
        panic!("シミュレーション時間が不正です");
    }
    if time_span <= 0.0 {
        time_span = 1.0;
    }

    let mut min_value = f64::MAX;
    let mut max_value = f64::MIN;
    for point in history_window {
        for value in [point.revenue, point.expense, point.debt] {
            if value < min_value {
                min_value = value;
            }
            if value > max_value {
                max_value = value;
            }
        }
    }
    if !min_value.is_finite() || !max_value.is_finite() {
        panic!("財政履歴に非有限値が含まれています");
    }
    if min_value == f64::MAX {
        panic!("財政履歴の集計に失敗しました");
    }
    if (max_value - min_value).abs() <= f64::EPSILON {
        max_value = min_value + 1.0;
    }
    let value_span = max_value - min_value;
    let scale_x = plot_width / time_span;
    let scale_y = plot_height / value_span;

    let map_x = |minutes: f64| -> f64 { left_margin + (minutes - min_time) * scale_x };
    let map_y = |value: f64| -> f64 { top_margin + (plot_height - (value - min_value) * scale_y) };

    let build_path = |value_fn: fn(&realgeopolitics_core::FiscalTrendPoint) -> f64| -> String {
        let mut iter = history_window.iter();
        let first = iter.next().expect("履歴の先頭取得に失敗しました");
        let mut path = format!(
            "M {:.2} {:.2}",
            map_x(first.simulation_minutes),
            map_y(value_fn(first))
        );
        for point in iter {
            let x = map_x(point.simulation_minutes);
            let y = map_y(value_fn(point));
            path.push_str(&format!(" L {:.2} {:.2}", x, y));
        }
        path
    };

    let revenue_path = build_path(|point| point.revenue);
    let expense_path = build_path(|point| point.expense);
    let debt_path = build_path(|point| point.debt);

    let grid_line_count = 4;
    let mut grid_lines = Vec::with_capacity(grid_line_count as usize);
    for idx in 0..=grid_line_count {
        let ratio = idx as f64 / grid_line_count as f64;
        let value = min_value + ratio * value_span;
        let y = map_y(value);
        grid_lines.push((y, value));
    }

    let latest_point = history_window.last().expect("履歴の末尾取得に失敗しました");

    html! {
        <div class="fiscal-chart">
            <svg viewBox={format!("0 0 {:.0} {:.0}", width, height)} preserveAspectRatio="none">
                <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", map_y(min_value))} x2={format!("{:.2}", left_margin + plot_width)} y2={format!("{:.2}", map_y(min_value))} stroke="#cccccc" stroke-width="1" />
                <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", top_margin)} x2={format!("{:.2}", left_margin)} y2={format!("{:.2}", top_margin + plot_height)} stroke="#cccccc" stroke-width="1" />
                { for grid_lines.iter().map(|(y, _)| {
                    html! { <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", y)} x2={format!("{:.2}", left_margin + plot_width)} y2={format!("{:.2}", y)} stroke="#eeeeee" stroke-width="0.5" /> }
                }) }
                <path d={revenue_path} stroke="#2e86de" stroke-width="2" fill="none" />
                <path d={expense_path} stroke="#c0392b" stroke-width="2" fill="none" stroke-dasharray="6 4" />
                <path d={debt_path} stroke="#27ae60" stroke-width="2" fill="none" stroke-dasharray="2 3" />
            </svg>
            <div class="chart-legend">
                <span class="legend-item revenue">{ format!("収入 {:.1}", snapshot.revenue) }</span>
                <span class="legend-item expense">{ format!("支出 {:.1}", snapshot.expense) }</span>
                <span class="legend-item debt">{ format!("債務 {:.1}", snapshot.debt) }</span>
            </div>
            <div class="chart-summary">
                <span>{ format!("シミュレーション時間 {:.1} 分", latest_point.simulation_minutes) }</span>
                <span>{ format!("現金準備 {:.1}", snapshot.cash_reserve) }</span>
                <span>{ format!("純キャッシュフロー {:.1}", snapshot.net_cash_flow) }</span>
            </div>
            <div class="chart-scale" style={format!("position: relative; height: {:.0}px; width: {:.0}px;", height, left_margin)}>
                { for grid_lines.iter().map(|(y, value)| {
                    html! {
                        <span class="scale-label" style={format!("position: absolute; top: {:.2}px; left: 0px;", y - 8.0)}>
                            { format!("{:.0}", value) }
                        </span>
                    }
                }) }
            </div>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
fn render_amount_input(
    label: &str,
    value: f64,
    country_idx: usize,
    field: AllocationField,
    callback: Callback<AllocationAmountChange>,
) -> Html {
    let onchange = {
        let callback = callback.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            {
                if let Ok(value) = input.value().parse::<f64>() {
                    callback.emit(AllocationAmountChange {
                        country_idx,
                        field,
                        value,
                    });
                }
            }
        })
    };

    html! {
        <div class="amount-row">
            <label>{ format!("{}: {:.1}%", label, value) }</label>
            <input type="number" min="0" step="0.5" value={format!("{:.1}", value)} oninput={onchange} />
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
fn render_core_toggle(
    enabled: bool,
    country_idx: usize,
    callback: Callback<CoreMinimumChange>,
) -> Html {
    let onchange = {
        let callback = callback.clone();
        Callback::from(move |event: Event| {
            if let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            {
                callback.emit(CoreMinimumChange {
                    country_idx,
                    enabled: input.checked(),
                });
            }
        })
    };

    html! {
        <label class="core-toggle">
            <input type="checkbox" checked={enabled} onchange={onchange} />
            { "コア支出を優先" }
        </label>
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

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use wasm_bindgen_test::*;
    use yew::Callback;

    #[wasm_bindgen_test]
    fn render_core_toggle_handles_checkbox_event() {
        let captured = Rc::new(RefCell::new(None));
        let sink = captured.clone();
        let callback = Callback::from(move |change: CoreMinimumChange| {
            sink.borrow_mut().replace(change.enabled);
        });

        let node = render_core_toggle(true, 0, callback);
        match node {
            yew::virtual_dom::VNode::VTag(_) | yew::virtual_dom::VNode::VComp(_) => {}
            other => panic!("予期しないノード種別: {:?}", other),
        }
        assert!(captured.borrow().is_none());
    }
}
#[cfg(target_arch = "wasm32")]
fn render_dashboard(
    gdp: f64,
    cash: f64,
    debt: f64,
    approval: f64,
    debt_ratio: f64,
    trend_points: &[FiscalTrendPoint],
) -> Html {
    if trend_points.is_empty() {
        panic!("トレンドデータが不足しています");
    }
    let width = 360.0;
    let height = 160.0;
    let left_margin = 36.0;
    let right_margin = 12.0;
    let top_margin = 18.0;
    let bottom_margin = 28.0;
    let plot_width = width - left_margin - right_margin;
    let plot_height = height - top_margin - bottom_margin;
    if plot_width <= 0.0 || plot_height <= 0.0 {
        panic!("トレンドグラフの描画領域が無効です");
    }

    let ratios: Vec<f64> = trend_points
        .iter()
        .map(|point| {
            if point.debt_ratio.is_finite() {
                point.debt_ratio.max(0.0)
            } else {
                200.0
            }
        })
        .collect();
    let mut min_ratio = ratios
        .iter()
        .copied()
        .fold(f64::MAX, |acc, value| acc.min(value));
    let mut max_ratio = ratios
        .iter()
        .copied()
        .fold(f64::MIN, |acc, value| acc.max(value));
    if ratios.len() == 1 {
        min_ratio = min_ratio.min(ratios[0] - 1.0);
        max_ratio = max_ratio.max(ratios[0] + 1.0);
    }
    if (max_ratio - min_ratio).abs() < 1.0 {
        max_ratio = max_ratio + 1.0;
        min_ratio = (min_ratio - 1.0).max(0.0);
    }
    let scale_x = if trend_points.len() > 1 {
        plot_width / ((trend_points.len() - 1) as f64)
    } else {
        0.0
    };
    let scale_y = plot_height / (max_ratio - min_ratio);
    let map_x = |index: usize| -> f64 {
        if trend_points.len() <= 1 {
            left_margin + plot_width / 2.0
        } else {
            left_margin + (index as f64) * scale_x
        }
    };
    let map_y = |ratio: f64| -> f64 { top_margin + (plot_height - (ratio - min_ratio) * scale_y) };

    let mut path = String::new();
    for (idx, ratio) in ratios.iter().enumerate() {
        let command = if idx == 0 { 'M' } else { 'L' };
        path.push_str(&format!(
            "{} {:.2} {:.2} ",
            command,
            map_x(idx),
            map_y(*ratio)
        ));
    }

    let reference_lines = 4;
    let mut grid_lines = Vec::new();
    for idx in 0..=reference_lines {
        let ratio = min_ratio + (idx as f64 / reference_lines as f64) * (max_ratio - min_ratio);
        let line_y = map_y(ratio);
        grid_lines.push((line_y, ratio));
    }

    let balance = cash - debt;
    let latest_ratio = *ratios.last().unwrap_or(&debt_ratio);

    html! {
        <section class="dashboard">
            <h2>{ "ダッシュボード" }</h2>
            <div class="metrics-cards">
                <div class="metric-card">
                    <span class="label">{ "GDP" }</span>
                    <span class="value">{ format_compact_number(gdp) }</span>
                </div>
                <div class="metric-card">
                    <span class="label">{ "バランスシート" }</span>
                    <span class="value">{ format_balance(balance) }</span>
                    <span class="sub">{ format!("現金 {:.1} / 債務 {:.1}", cash, debt) }</span>
                </div>
                <div class="metric-card">
                    <span class="label">{ "債務比率" }</span>
                    <span class="value">{ format_ratio(debt_ratio) }</span>
                </div>
                <div class="metric-card">
                    <span class="label">{ "世論指数" }</span>
                    <span class="value">{ format!("{:.0}", approval) }</span>
                </div>
            </div>
            <div class="trend-chart">
                <h3>{ "債務比率トレンド (直近12 Tick)" }</h3>
                <svg viewBox={format!("0 0 {:.0} {:.0}", width, height)} preserveAspectRatio="none">
                    <rect x="0" y="0" width={format!("{:.0}", width)} height={format!("{:.0}", height)} fill="none" stroke="#dddddd" stroke-width="0.5" />
                    <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", top_margin)} x2={format!("{:.2}", left_margin)} y2={format!("{:.2}", top_margin + plot_height)} stroke="#cccccc" stroke-width="1" />
                    <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", top_margin + plot_height)} x2={format!("{:.2}", left_margin + plot_width)} y2={format!("{:.2}", top_margin + plot_height)} stroke="#cccccc" stroke-width="1" />
                    { for grid_lines.iter().map(|(line_y, value)| {
                        html! {
                            <g>
                                <line x1={format!("{:.2}", left_margin)} y1={format!("{:.2}", line_y)} x2={format!("{:.2}", left_margin + plot_width)} y2={format!("{:.2}", line_y)} stroke="#eeeeee" stroke-width="0.5" />
                                <text x={format!("{:.2}", 2.0)} y={format!("{:.2}", line_y + 4.0)} class="axis-label">{ format!("{:.0}%", value) }</text>
                            </g>
                        }
                    }) }
                    <path d={path} stroke="#8e44ad" stroke-width="2" fill="none" />
                    { for ratios.iter().enumerate().map(|(idx, ratio)| {
                        html! {
                            <circle cx={format!("{:.2}", map_x(idx))} cy={format!("{:.2}", map_y(*ratio))} r="2.5" fill="#8e44ad" />
                        }
                    }) }
                </svg>
                <div class="trend-summary">
                    <span>{ format!("最新債務比率: {}", format_ratio(latest_ratio)) }</span>
                    <span>{ format!("データ点: {}", trend_points.len()) }</span>
                </div>
            </div>
        </section>
    }
}
#[cfg(target_arch = "wasm32")]
fn format_compact_number(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{:.1}", value)
    }
}

#[cfg(target_arch = "wasm32")]
fn format_balance(value: f64) -> String {
    if value >= 0.0 {
        format!("+{:.1}", value)
    } else {
        format!("{:.1}", value)
    }
}

#[cfg(target_arch = "wasm32")]
fn format_ratio(value: f64) -> String {
    if value.is_finite() {
        format!("{:.1}%", value)
    } else {
        "∞%".to_string()
    }
}
