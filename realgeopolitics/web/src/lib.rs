#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use realgeopolitics_core::{Action, CountryDefinition, GameState};
use serde_json::Error as SerdeError;

#[cfg(target_arch = "wasm32")]
use yew::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlSelectElement;

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

#[cfg(target_arch = "wasm32")]
#[derive(Clone, PartialEq)]
enum ActionKind {
    None,
    Infrastructure,
    Military,
    Welfare,
    Diplomacy,
}

#[cfg(target_arch = "wasm32")]
impl ActionKind {
    fn from_value(value: &str) -> Self {
        match value {
            "infra" => ActionKind::Infrastructure,
            "mil" => ActionKind::Military,
            "wel" => ActionKind::Welfare,
            "dip" => ActionKind::Diplomacy,
            _ => ActionKind::None,
        }
    }

    fn as_value(&self) -> &'static str {
        match self {
            ActionKind::None => "none",
            ActionKind::Infrastructure => "infra",
            ActionKind::Military => "mil",
            ActionKind::Welfare => "wel",
            ActionKind::Diplomacy => "dip",
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, PartialEq)]
struct ActionForm {
    kind: ActionKind,
    target_idx: usize,
}

#[cfg(target_arch = "wasm32")]
impl Default for ActionForm {
    fn default() -> Self {
        Self {
            kind: ActionKind::None,
            target_idx: 0,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn action_form_from_state(game: &GameState, actor_idx: usize) -> ActionForm {
    let mut form = ActionForm::default();
    if let Some(country) = game.countries().get(actor_idx) {
        if let Some(action) = country.planned_action() {
            match action {
                Action::Infrastructure => {
                    form.kind = ActionKind::Infrastructure;
                }
                Action::MilitaryDrill => {
                    form.kind = ActionKind::Military;
                }
                Action::WelfarePackage => {
                    form.kind = ActionKind::Welfare;
                }
                Action::Diplomacy { target } => {
                    form.kind = ActionKind::Diplomacy;
                    if let Some(idx) = game.find_country_index(target) {
                        form.target_idx = idx;
                    }
                }
            }
        }
    }
    form
}

#[cfg(target_arch = "wasm32")]
fn diplomacy_default_target(game: &GameState, actor_idx: usize) -> usize {
    for (idx, _) in game.countries().iter().enumerate() {
        if idx != actor_idx {
            return idx;
        }
    }
    actor_idx
}

#[cfg(target_arch = "wasm32")]
#[function_component(App)]
fn app() -> Html {
    let initial_definitions = load_default_definitions().expect("国データの読み込みに失敗しました");
    let game = use_mut_ref(|| {
        GameState::from_definitions(initial_definitions).expect("国データの初期化に失敗しました")
    });

    let refresh = use_state(|| 0u32);
    let selected_country = use_state(|| 0usize);
    let action_form = use_state(ActionForm::default);
    let message = use_state(|| Option::<String>::None);
    let reports = use_state(Vec::<String>::new);

    {
        let game = game.clone();
        let action_form = action_form.clone();
        let selected_country = selected_country.clone();
        use_effect_with(selected_country.clone(), move |idx| {
            let game_ref = game.borrow();
            let form = action_form_from_state(&game_ref, **idx);
            action_form.set(form);
            || ()
        });
    }

    let force_refresh = {
        let refresh = refresh.clone();
        Callback::from(move |_| {
            refresh.set(refresh.wrapping_add(1));
        })
    };

    let set_error = {
        let message = message.clone();
        Callback::from(move |err: String| {
            message.set(Some(err));
        })
    };

    let clear_error = {
        let message = message.clone();
        Callback::from(move |_| {
            message.set(None);
        })
    };

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

    let on_action_change = {
        let action_form = action_form.clone();
        let game = game.clone();
        let selected_country = selected_country.clone();
        Callback::from(move |event: Event| {
            let mut updated = (*action_form).clone();
            if let Some(select) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlSelectElement>().ok())
            {
                updated.kind = ActionKind::from_value(&select.value());
                if matches!(updated.kind, ActionKind::Diplomacy) {
                    let game_ref = game.borrow();
                    updated.target_idx = diplomacy_default_target(&game_ref, *selected_country);
                }
            }
            action_form.set(updated);
        })
    };

    let on_target_change = {
        let action_form = action_form.clone();
        Callback::from(move |event: Event| {
            if let Some(select) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlSelectElement>().ok())
            {
                if let Ok(idx) = select.value().parse::<usize>() {
                    let mut updated = (*action_form).clone();
                    updated.target_idx = idx;
                    action_form.set(updated);
                }
            }
        })
    };

    let on_plan_action = {
        let game = game.clone();
        let selected_country = selected_country.clone();
        let action_form = action_form.clone();
        let set_error = set_error.clone();
        let clear_error = clear_error.clone();
        let force_refresh = force_refresh.clone();
        Callback::from(move |_| {
            let actor_idx = *selected_country;
            let current_form = (*action_form).clone();
            if matches!(current_form.kind, ActionKind::None) {
                set_error.emit("行動種別を選択してください。".to_string());
                return;
            }
            let action = {
                let game_ref = game.borrow();
                match current_form.kind {
                    ActionKind::Infrastructure => Some(Action::Infrastructure),
                    ActionKind::Military => Some(Action::MilitaryDrill),
                    ActionKind::Welfare => Some(Action::WelfarePackage),
                    ActionKind::Diplomacy => {
                        let countries = game_ref.countries();
                        if current_form.target_idx >= countries.len() {
                            None
                        } else if current_form.target_idx == actor_idx {
                            None
                        } else {
                            Some(Action::Diplomacy {
                                target: countries[current_form.target_idx].name.clone(),
                            })
                        }
                    }
                    ActionKind::None => None,
                }
            };

            if let Some(action) = action {
                let mut game_mut = game.borrow_mut();
                match game_mut.plan_action(actor_idx, action) {
                    Ok(_) => {
                        clear_error.emit(());
                        let updated_form = action_form_from_state(&game_mut, actor_idx);
                        action_form.set(updated_form);
                        force_refresh.emit(());
                    }
                    Err(err) => set_error.emit(err.to_string()),
                }
            } else {
                set_error.emit("外交対象の選択が正しくありません。".to_string());
            }
        })
    };

    let on_cancel_action = {
        let game = game.clone();
        let selected_country = selected_country.clone();
        let clear_error = clear_error.clone();
        let set_error = set_error.clone();
        let force_refresh = force_refresh.clone();
        let action_form = action_form.clone();
        Callback::from(move |_| {
            let actor_idx = *selected_country;
            let mut game_mut = game.borrow_mut();
            match game_mut.cancel_action(actor_idx) {
                Ok(_) => {
                    action_form.set(ActionForm::default());
                    clear_error.emit(());
                    force_refresh.emit(());
                }
                Err(err) => set_error.emit(err.to_string()),
            }
        })
    };

    let on_advance_turn = {
        let game = game.clone();
        let reports_state = reports.clone();
        let clear_error = clear_error.clone();
        let set_error = set_error.clone();
        let force_refresh = force_refresh.clone();
        Callback::from(move |_| {
            let mut game_mut = game.borrow_mut();
            match game_mut.advance_turn() {
                Ok(result) => {
                    reports_state.set(result);
                    clear_error.emit(());
                    force_refresh.emit(());
                }
                Err(err) => set_error.emit(err.to_string()),
            }
        })
    };

    let game_snapshot = game.borrow();
    let countries = game_snapshot.countries();
    let selected_idx = (*selected_country).min(countries.len().saturating_sub(1));

    let relations: Vec<(String, i32)> = if let Some(country) = countries.get(selected_idx) {
        let mut pairs: Vec<_> = country
            .relations()
            .iter()
            .map(|(name, value)| (name.clone(), *value))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    } else {
        Vec::new()
    };

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

    let action_select_disabled =
        countries.len() <= 1 && matches!((*action_form).kind, ActionKind::Diplomacy);

    html! {
        <div class="app">
            <header>
                <h1>{ "リアル・ジオポリティクス シミュレーター" }</h1>
                <p>{ format!("ターン {}", game_snapshot.turn()) }</p>
            </header>

            { message_view }

            <section class="controls">
                <label>
                    { "対象国" }
                    <select onchange={on_country_change.clone()} value={selected_idx.to_string()}>
                        { for countries.iter().enumerate().map(|(idx, country)| {
                            html! { <option value={idx.to_string()}>{ format!("{}: {}", idx + 1, country.name) }</option> }
                        }) }
                    </select>
                </label>

                <label>
                    { "行動" }
                    <select onchange={on_action_change.clone()} value={(*action_form).kind.as_value()}>
                        <option value="none">{ "選択なし" }</option>
                        <option value="infra">{ "インフラ投資" }</option>
                        <option value="mil">{ "軍事演習" }</option>
                        <option value="wel">{ "社会福祉" }</option>
                        <option value="dip" disabled={action_select_disabled}>{ "外交ミッション" }</option>
                    </select>
                </label>

                <label>
                    { "外交相手" }
                    <select onchange={on_target_change.clone()} value={(*action_form).target_idx.to_string()} disabled={!matches!((*action_form).kind, ActionKind::Diplomacy)}>
                        { for countries.iter().enumerate().map(|(idx, country)| {
                            let disabled = idx == selected_idx;
                            html! { <option value={idx.to_string()} disabled={disabled}>{ &country.name }</option> }
                        }) }
                    </select>
                </label>

                <div class="control-buttons">
                    <button onclick={on_plan_action}>{ "行動を設定" }</button>
                    <button onclick={on_cancel_action}>{ "予約解除" }</button>
                    <button class="advance" onclick={on_advance_turn}>{ "ターンを進める" }</button>
                </div>
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
                            <th>{ "資源" }</th>
                            <th>{ "次ターン行動" }</th>
                        </tr>
                    </thead>
                    <tbody>
                        { for countries.iter().enumerate().map(|(idx, country)| {
                            let planned = country.planned_action().map(|action| match action {
                                Action::Diplomacy { target } => format!("{} ({})", action.label(), target),
                                _ => action.label().to_string(),
                            }).unwrap_or_else(|| "未設定".to_string());
                            let row_class = if idx == selected_idx { "selected" } else { "" };
                            html! {
                                <tr class={row_class}>
                                    <td>{ idx + 1 }</td>
                                    <td>{ &country.name }</td>
                                    <td>{ &country.government }</td>
                                    <td>{ format!("{:.1}", country.gdp) }</td>
                                    <td>{ country.stability }</td>
                                    <td>{ country.military }</td>
                                    <td>{ country.approval }</td>
                                    <td>{ format!("{:.1}", country.budget) }</td>
                                    <td>{ country.resources }</td>
                                    <td>{ planned }</td>
                                </tr>
                            }
                        }) }
                    </tbody>
                </table>
            </section>

            <section class="relations">
                <h2>{ format!("{} の外交関係", countries.get(selected_idx).map(|c| c.name.as_str()).unwrap_or("-")) }</h2>
                <ul>
                    { for relations.iter().map(|(partner, value)| {
                        html! { <li key={partner.clone()}>{ format!("{}: {}", partner, value) }</li> }
                    }) }
                </ul>
            </section>

            <section class="reports">
                <h2>{ "最新ターンのレポート" }</h2>
                <ul>{ reports_view }</ul>
            </section>
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
