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
            let debt_service = parse_percentage(parts.next(), "債務返済")?;
            let administration = parse_percentage(parts.next(), "行政維持")?;
            let research = parse_percentage(parts.next(), "研究開発")?;
            let ensure_core = match parts.next() {
                Some(flag) if flag.eq_ignore_ascii_case("core") => true,
                Some(flag) if flag.eq_ignore_ascii_case("nocore") => false,
                Some(other) => {
                    return Err(anyhow!(
                        "未知のフラグです: {} (core または nocore を指定してください)",
                        other
                    ));
                }
                None => true,
            };
            let allocation = BudgetAllocation::new(
                infra,
                military,
                welfare,
                diplomacy,
                debt_service,
                administration,
                research,
                ensure_core,
            )?;
            game.update_allocations(idx, allocation)?;
            println!(
                "{} の予算配分を更新しました (合計 {:.1}%)",
                game.countries()[idx].name,
                allocation.total_percentage()
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
            let multiplier = game.time_multiplier();
            let reports = game.tick_minutes(minutes)?;
            print_reports(minutes * multiplier, reports);
            Ok(())
        }
        "speed" => {
            let token = parts
                .next()
                .ok_or_else(|| anyhow!("新しい時間倍率を指定してください。"))?;
            let multiplier = parse_speed(token)?;
            game.set_time_multiplier(multiplier)?;
            println!("時間倍率を x{:.2} に設定しました。", game.time_multiplier());
            Ok(())
        }
        "industry" => {
            let sub = parts
                .next()
                .ok_or_else(|| {
                    anyhow!("industry サブコマンドを指定してください (例: subsidize)。")
                })?
                .to_ascii_lowercase();
            match sub.as_str() {
                "subsidize" => {
                    let sector_token = parts.next().ok_or_else(|| {
                        anyhow!("セクターを category:key 形式または一意なキーで指定してください。")
                    })?;
                    let percent_text = parts
                        .next()
                        .ok_or_else(|| anyhow!("補助金割合(%)を指定してください。"))?;
                    let percent: f64 = percent_text
                        .parse()
                        .map_err(|_| anyhow!("補助金割合は数値で指定してください。"))?;
                    let overview = game.apply_industry_subsidy(sector_token, percent)?;
                    println!(
                        "{} ({}) に補助金 {:.1}% を設定しました。直近コスト {:.1} / 生産量 {:.1}",
                        overview.name,
                        overview.id.category,
                        overview.subsidy_percent,
                        overview.last_cost,
                        overview.last_output
                    );
                    Ok(())
                }
                other => bail!("未知の industry サブコマンドです: {}", other),
            }
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
    println!("speed コマンドで時間倍率を slow/normal/fast などに変更できます。");
}

fn print_help() {
    println!("利用可能なコマンド:");
    println!("  overview              主要指標と配分を一覧表示");
    println!("  inspect <国>          選択した国の詳細と外交関係を表示");
    println!("  set <国> <infra> <mil> <welfare> <diplo> <debt> <admin> <research> [core|nocore]");
    println!("                       各カテゴリのGDP比率(%)を入力 (core で必須支出を優先)");
    println!("  tick <分>             指定した分だけシミュレーションを進める");
    println!("  speed <倍率|slow|normal|fast>  時間倍率を変更");
    println!("  industry subsidize <sector> <percent>  指定セクターへ補助金(%)を設定");
    println!("  quit                  終了");
}

fn print_overview(game: &GameState) {
    let next_event = game
        .next_event_minutes()
        .map(|m| format!("{:.1} 分", m as f64))
        .unwrap_or_else(|| "未定".to_string());
    println!(
        "シミュレーション時間: {:.1} 分 (倍率 x{:.2}) / 次イベントまで: {} / 資源価格 {:.1}",
        game.simulation_minutes(),
        game.time_multiplier(),
        next_event,
        game.commodity_price()
    );
    println!(
        "ID | {:<18} | {:<22} | {:>9} | {:>4} | {:>4} | {:>4} | {:>9} | alloc%(i/m/w/d/debt/adm/res)",
        "国名", "政体", "GDP", "安定", "軍事", "支持", "予算"
    );
    for (idx, country) in game.countries().iter().enumerate() {
        let alloc = country.allocations();
        println!(
            "{:>2} | {:<18} | {:<22} | {:>9.1} | {:>4} | {:>4} | {:>4} | {:>9.1} | {:>6.1}%/{:>6.1}%/{:>6.1}%/{:>6.1}%/{:>6.1}%/{:>6.1}%/{:>6.1}%",
            idx + 1,
            country.name,
            country.government,
            country.gdp,
            country.stability,
            country.military,
            country.approval,
            country.cash_reserve(),
            alloc.infrastructure,
            alloc.military,
            alloc.welfare,
            alloc.diplomacy,
            alloc.debt_service,
            alloc.administration,
            alloc.research
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
    println!("予算残高: {:.1}", country.cash_reserve());
    println!(
        "今期収支: 収入 {:.1} / 支出 {:.1} / 差額 {:.1}",
        country.total_revenue(),
        country.total_expense(),
        country.net_cash_flow()
    );
    let tax = country.tax_policy();
    println!(
        "税制: 所得 {:.1}% / 法人 {:.1}% / 消費 {:.1}% (控除 {:.1}, 次期繰越 {:.1})",
        tax.income_rate * 100.0,
        tax.corporate_rate * 100.0,
        tax.consumption_rate * 100.0,
        tax.deductions,
        tax.pending_revenue()
    );
    println!("資源指数: {}", country.resources);
    println!(
        "予算配分 (GDP比%): インフラ {:.1}% / 軍事 {:.1}% / 福祉 {:.1}% / 外交 {:.1}% / 債務 {:.1}% / 行政 {:.1}% / 研究 {:.1}{}",
        alloc.infrastructure,
        alloc.military,
        alloc.welfare,
        alloc.diplomacy,
        alloc.debt_service,
        alloc.administration,
        alloc.research,
        if alloc.ensure_core_minimum {
            " / コア維持"
        } else {
            ""
        }
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
    let value = value.ok_or_else(|| anyhow!("{}予算の割合(%)を指定してください。", label))?;
    let percent: f64 = value
        .parse()
        .map_err(|_| anyhow!("{}予算割合は数値で指定してください。", label))?;
    ensure!(percent.is_finite(), "{}予算割合が不正です", label);
    ensure!(
        percent >= 0.0,
        "{}予算割合は0以上で指定してください。",
        label
    );
    Ok(percent)
}

fn parse_speed(token: &str) -> Result<f64> {
    let lower = token.to_ascii_lowercase();
    let multiplier = match lower.as_str() {
        "slow" | "low" => 0.5,
        "normal" | "std" | "standard" => 1.0,
        "fast" | "high" => 2.0,
        "max" => 4.0,
        _ => token
            .parse::<f64>()
            .map_err(|_| anyhow!("時間倍率は slow/normal/fast もしくは数値で指定してください。"))?,
    };
    ensure!(
        multiplier.is_finite() && multiplier > 0.0,
        "時間倍率は正の有限値で指定してください。"
    );
    Ok(multiplier)
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
