use std::io::{self, BufRead, Write};

use anyhow::{Context, Result, anyhow, bail};
use realgeopolitics_core::{Action, GameState};

pub fn run(game: &mut GameState) -> Result<()> {
    print_intro(game);
    let stdin = io::stdin();

    loop {
        print!("ターン{}> ", game.turn());
        io::stdout()
            .flush()
            .context("プロンプトのフラッシュに失敗しました")?;

        let mut line = String::new();
        let bytes = stdin
            .lock()
            .read_line(&mut line)
            .context("入力の読み込みに失敗しました")?;

        if bytes == 0 {
            println!("入力が終了したためゲームを終了します。");
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Err(error) = dispatch_command(game, trimmed) {
            println!("エラー: {error}");
        }
    }
}

fn dispatch_command(game: &mut GameState, input: &str) -> Result<()> {
    let mut parts = input.split_whitespace();
    let command = parts
        .next()
        .ok_or_else(|| anyhow!("コマンドが指定されていません。"))?
        .to_ascii_lowercase();

    match command.as_str() {
        "help" | "?" => {
            print_help();
            Ok(())
        }
        "overview" | "ov" => {
            print_overview(game);
            Ok(())
        }
        "inspect" | "show" => {
            let country_token = parts
                .next()
                .ok_or_else(|| anyhow!("国を指定してください。"))?;
            let idx = resolve_country_index(game, country_token)?;
            print_country_details(game, idx);
            Ok(())
        }
        "plan" => {
            let country_token = parts
                .next()
                .ok_or_else(|| anyhow!("行動を設定する国を指定してください。"))?;
            let action_token = parts
                .next()
                .ok_or_else(|| anyhow!("行動種別を指定してください。"))?;
            let extra = parts.next();
            let idx = resolve_country_index(game, country_token)?;
            let action = parse_action(game, idx, action_token, extra)?;
            game.plan_action(idx, action)?;
            println!("{} の行動を設定しました。", game.countries()[idx].name);
            Ok(())
        }
        "cancel" => {
            let country_token = parts
                .next()
                .ok_or_else(|| anyhow!("キャンセル対象の国を指定してください。"))?;
            let idx = resolve_country_index(game, country_token)?;
            game.cancel_action(idx)?;
            println!(
                "{} の行動予約を取り消しました。",
                game.countries()[idx].name
            );
            Ok(())
        }
        "status" => {
            print_planned_actions(game);
            Ok(())
        }
        "end" | "advance" => {
            let reports = game.advance_turn()?;
            println!("--- ターン{} の結果 ---", game.turn());
            for report in reports {
                println!("- {report}");
            }
            println!("--------------------------");
            Ok(())
        }
        "quit" | "exit" => {
            println!("ゲームを終了します。");
            std::process::exit(0);
        }
        other => {
            bail!("未知のコマンドです: {other}. help で一覧を確認してください。");
        }
    }
}

fn print_intro(game: &GameState) {
    println!("リアル・ジオポリティクス シミュレーターへようこそ。");
    println!("現在 {} ヶ国が参加しています。", game.countries().len());
    println!("コマンド例: overview / inspect 1 / plan 2 infrastructure / plan 1 diplomacy 3 / end");
    println!("help で利用可能なコマンド一覧を表示します。");
}

fn print_help() {
    println!("利用可能なコマンド:");
    println!("  overview              主要指標の一覧を表示");
    println!("  inspect <国>          詳細情報と関係を表示");
    println!("  plan <国> <行動>      次のターンの行動を設定");
    println!(
        "                         行動: infrastructure | military | welfare | diplomacy <相手>"
    );
    println!("  cancel <国>           予約した行動をキャンセル");
    println!("  status                各国の予約済み行動を確認");
    println!("  end                   行動を実行してターンを進める");
    println!("  quit                  ゲームを終了");
}

fn print_overview(game: &GameState) {
    println!(
        "ID | {:<18} | {:<24} | {:>9} | {:>4} | {:>4} | {:>4} | {:>9}",
        "国名", "政体", "GDP", "安定", "軍事", "支持", "予算"
    );
    for (idx, country) in game.countries().iter().enumerate() {
        println!(
            "{:>2} | {:<18} | {:<24} | {:>9.1} | {:>4} | {:>4} | {:>4} | {:>9.1}",
            idx + 1,
            country.name,
            country.government,
            country.gdp,
            country.stability,
            country.military,
            country.approval,
            country.budget
        );
    }
}

fn print_country_details(game: &GameState, idx: usize) {
    let country = &game.countries()[idx];
    println!("-- {} の状況 --", country.name);
    println!("政体: {}", country.government);
    println!("人口: {:.1} 百万人", country.population_millions);
    println!("GDP: {:.1} 億ドル", country.gdp);
    println!("安定度: {}", country.stability);
    println!("軍事力: {}", country.military);
    println!("国民支持率: {}", country.approval);
    println!("予算残高: {:.1}", country.budget);
    println!("資源指数: {}", country.resources);
    if let Some(action) = country.planned_action() {
        println!(
            "次ターン行動: {} (必要予算 {:.1})",
            action.label(),
            action.cost()
        );
    } else {
        println!("次ターン行動: 未設定");
    }

    println!("外交関係:");
    let mut relations: Vec<_> = country.relations().iter().collect();
    relations.sort_by(|a, b| a.0.cmp(b.0));
    for (partner, value) in relations {
        println!("  - {:<20}: {:>4}", partner, value);
    }
}

fn print_planned_actions(game: &GameState) {
    println!("予約済み行動:");
    for (idx, country) in game.countries().iter().enumerate() {
        if let Some(action) = country.planned_action() {
            println!(
                "{:>2}: {} -> {} (必要 {:.1})",
                idx + 1,
                country.name,
                action.label(),
                action.cost()
            );
        } else {
            println!("{:>2}: {} -> 未設定", idx + 1, country.name);
        }
    }
}

fn resolve_country_index(game: &GameState, token: &str) -> Result<usize> {
    game.find_country_index(token).ok_or_else(|| {
        anyhow!(
            "国を特定できませんでした: {} (番号か完全な国名を入力してください)",
            token
        )
    })
}

fn parse_action(
    game: &GameState,
    actor_idx: usize,
    action_token: &str,
    extra: Option<&str>,
) -> Result<Action> {
    match action_token.to_ascii_lowercase().as_str() {
        "infrastructure" | "infra" => Ok(Action::Infrastructure),
        "military" | "drill" => Ok(Action::MilitaryDrill),
        "welfare" | "social" => Ok(Action::WelfarePackage),
        "diplomacy" | "diplo" => {
            let target_token = extra.ok_or_else(|| anyhow!("外交相手を指定してください。"))?;
            let target_idx = resolve_country_index(game, target_token)?;
            if target_idx == actor_idx {
                bail!("自国を外交対象に選ぶことはできません。");
            }
            Ok(Action::Diplomacy {
                target: game.countries()[target_idx].name.clone(),
            })
        }
        other => bail!("未知の行動です: {other}"),
    }
}
