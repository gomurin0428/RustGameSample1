use std::io::{self, BufRead, Write};

use anyhow::{Context, Result, anyhow, bail, ensure};
use realgeopolitics_core::{BudgetAllocation, GameState};

pub fn run(game: &mut GameState) -> Result<()> {
    print_intro(game);
    let stdin = io::stdin();

    loop {
        print!("t={:.1}分> ", game.simulation_minutes());
        io::stdout()
            .flush()
            .context("プロンプトのフラッシュに失敗しました")?;

        let mut line = String::new();
        let bytes = stdin
            .lock()
            .read_line(&mut line)
            .context("入力の読み込みに失敗しました")?;

        if bytes == 0 {
            println!("入力が終了したためシミュレーションを終了します。");
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
            let token = parts
                .next()
                .ok_or_else(|| anyhow!("国を指定してください。"))?;
            let idx = resolve_country_index(game, token)?;
            print_country_details(game, idx);
            Ok(())
        }
        "set" => {
            let token = parts
                .next()
                .ok_or_else(|| anyhow!("国を指定してください。"))?;
            let idx = resolve_country_index(game, token)?;
            let infra = parse_percentage(parts.next(), "インフラ")?;
            let military = parse_percentage(parts.next(), "軍事")?;
            let welfare = parse_percentage(parts.next(), "福祉")?;
            let diplomacy = parse_percentage(parts.next(), "外交")?;
            let allocation =
                BudgetAllocation::from_percentages(infra, military, welfare, diplomacy)?;
            game.update_allocations(idx, allocation)?;
            println!(
                "{} の予算配分を更新しました (合計 {:.1}%)",
                game.countries()[idx].name,
                allocation.total() * 100.0
            );
            Ok(())
        }
        "tick" => {
            let minutes = parts
                .next()
                .ok_or_else(|| anyhow!("経過させる分数を指定してください。"))?;
            let minutes: f64 = minutes
                .parse()
                .map_err(|_| anyhow!("分数は数値で指定してください。"))?;
            let reports = game.tick_minutes(minutes)?;
            print_reports(minutes, reports);
            Ok(())
        }
        "quit" | "exit" => {
            println!("シミュレーションを終了します。");
            std::process::exit(0);
        }
        other => bail!("未知のコマンドです: {other}. help で一覧を確認してください。"),
    }
}

fn print_intro(game: &GameState) {
    println!("リアル・ジオポリティクス シミュレーター (リアルタイム版) へようこそ。");
    println!("現在 {} ヶ国が監視対象です。", game.countries().len());
    println!("help で利用可能なコマンド一覧を確認できます。");
}

fn print_help() {
    println!("利用可能なコマンド:");
    println!("  overview              主要指標と配分を一覧表示");
    println!("  inspect <国>          選択した国の詳細と外交関係を表示");
    println!("  set <国> <i> <m> <w> <d>   予算配分を百分率で設定 (合計100%以内)");
    println!("  tick <分>             指定した分だけシミュレーションを進める");
    println!("  quit                  終了");
}

fn print_overview(game: &GameState) {
    println!(
        "ID | {:<18} | {:<22} | {:>9} | {:>4} | {:>4} | {:>4} | {:>9} | alloc(i/m/w/d)",
        "国名", "政体", "GDP", "安定", "軍事", "支持", "予算"
    );
    for (idx, country) in game.countries().iter().enumerate() {
        let alloc = country.allocations();
        println!(
            "{:>2} | {:<18} | {:<22} | {:>9.1} | {:>4} | {:>4} | {:>4} | {:>9.1} | {:>4.0}/{:>4.0}/{:>4.0}/{:>4.0}",
            idx + 1,
            country.name,
            country.government,
            country.gdp,
            country.stability,
            country.military,
            country.approval,
            country.budget,
            alloc.infrastructure * 100.0,
            alloc.military * 100.0,
            alloc.welfare * 100.0,
            alloc.diplomacy * 100.0
        );
    }
}

fn print_country_details(game: &GameState, idx: usize) {
    let country = &game.countries()[idx];
    let alloc = country.allocations();
    println!("-- {} の状況 --", country.name);
    println!("政体: {}", country.government);
    println!("人口: {:.1} 百万人", country.population_millions);
    println!("GDP: {:.1} 億ドル", country.gdp);
    println!("安定度: {}", country.stability);
    println!("軍事力: {}", country.military);
    println!("国民支持率: {}", country.approval);
    println!("予算残高: {:.1}", country.budget);
    println!("資源指数: {}", country.resources);
    println!(
        "予算配分: インフラ {:.1}% / 軍事 {:.1}% / 福祉 {:.1}% / 外交 {:.1}%",
        alloc.infrastructure * 100.0,
        alloc.military * 100.0,
        alloc.welfare * 100.0,
        alloc.diplomacy * 100.0
    );
    println!("外交関係:");
    let mut relations: Vec<_> = country.relations.iter().collect();
    relations.sort_by(|a, b| a.0.cmp(b.0));
    for (partner, value) in relations {
        println!("  - {:<20}: {:>4}", partner, value);
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

fn parse_percentage(value: Option<&str>, label: &str) -> Result<f64> {
    let value = value.ok_or_else(|| anyhow!("{}予算の百分率を指定してください。", label))?;
    let percent: f64 = value
        .parse()
        .map_err(|_| anyhow!("{}予算は数値で指定してください。", label))?;
    ensure!(percent >= 0.0, "{}予算は0以上で指定してください。", label);
    Ok(percent)
}

fn print_reports(minutes: f64, reports: Vec<String>) {
    if reports.is_empty() {
        println!("{:.1} 分経過: 変化は特にありません。", minutes);
    } else {
        println!("{:.1} 分経過のレポート:", minutes);
        for report in reports {
            println!("- {report}");
        }
    }
}
