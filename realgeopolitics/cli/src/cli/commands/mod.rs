use std::collections::HashMap;
use std::io;
use std::process;

use anyhow::{Result, anyhow, bail};
use realgeopolitics_core::{BudgetAllocation, GameState};

use super::{
    parse_percentage, parse_speed, print_country_details, print_help, print_overview,
    print_reports, resolve_country_index,
};

pub struct Context<'a> {
    game: &'a mut GameState,
}

impl<'a> Context<'a> {
    pub fn new(game: &'a mut GameState) -> Self {
        Self { game }
    }

    pub fn game(&self) -> &GameState {
        &*self.game
    }

    pub fn game_mut(&mut self) -> &mut GameState {
        &mut *self.game
    }
}

pub struct Args<'a> {
    tokens: Vec<&'a str>,
    index: usize,
}

impl<'a> Args<'a> {
    pub fn new(tokens: Vec<&'a str>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn next(&mut self) -> Option<&'a str> {
        if self.index >= self.tokens.len() {
            return None;
        }
        let value = self.tokens[self.index];
        self.index += 1;
        Some(value)
    }

    pub fn next_required(&mut self, message: &str) -> Result<&'a str> {
        self.next().ok_or_else(|| anyhow!(message.to_owned()))
    }
}

pub trait Command {
    fn name() -> &'static str;
    fn execute(ctx: &mut Context<'_>, args: Args<'_>) -> Result<()>;
}

type CommandFn = for<'a> fn(&mut Context<'a>, Args<'a>) -> Result<()>;

pub struct CommandRegistry {
    handlers: HashMap<&'static str, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register<C: Command>(&mut self) {
        let name = C::name();
        if self.handlers.insert(name, C::execute).is_some() {
            panic!("重複したコマンド登録です: {name}");
        }
    }
    pub fn dispatch<'a>(&self, command: &str, ctx: &mut Context<'a>, args: Args<'a>) -> Result<()> {
        if let Some(handler) = self.handlers.get(command) {
            handler(ctx, args)
        } else {
            bail!("未対応のコマンドです: {command}. help で一覧を確認してください。");
        }
    }

    pub fn execute_input<'a>(&self, ctx: &mut Context<'a>, input: &'a str) -> Result<()> {
        let mut parts = input.split_whitespace();
        let Some(head) = parts.next() else {
            return Err(anyhow!("コマンドが指定されていません。"));
        };
        let command_name = head.to_ascii_lowercase();
        let args = Args::new(parts.collect());
        self.dispatch(command_name.as_str(), ctx, args)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register::<HelpCommand>();
        registry.register::<HelpAliasCommand>();
        registry.register::<OverviewCommand>();
        registry.register::<OverviewAliasCommand>();
        registry.register::<InspectCommand>();
        registry.register::<ShowCommand>();
        registry.register::<SetCommand>();
        registry.register::<TickCommand>();
        registry.register::<SpeedCommand>();
        registry.register::<IndustryCommand>();
        registry.register::<QuitCommand>();
        registry.register::<ExitCommand>();
        registry
    }
}
pub struct HelpCommand;

impl Command for HelpCommand {
    fn name() -> &'static str {
        "help"
    }

    fn execute(_ctx: &mut Context<'_>, _args: Args<'_>) -> Result<()> {
        print_help();
        Ok(())
    }
}

pub struct HelpAliasCommand;

impl Command for HelpAliasCommand {
    fn name() -> &'static str {
        "?"
    }

    fn execute(ctx: &mut Context<'_>, args: Args<'_>) -> Result<()> {
        HelpCommand::execute(ctx, args)
    }
}

pub struct OverviewCommand;

impl Command for OverviewCommand {
    fn name() -> &'static str {
        "overview"
    }

    fn execute(ctx: &mut Context<'_>, _args: Args<'_>) -> Result<()> {
        print_overview(ctx.game());
        Ok(())
    }
}
pub struct OverviewAliasCommand;

impl Command for OverviewAliasCommand {
    fn name() -> &'static str {
        "ov"
    }

    fn execute(ctx: &mut Context<'_>, args: Args<'_>) -> Result<()> {
        OverviewCommand::execute(ctx, args)
    }
}

pub struct InspectCommand;

impl Command for InspectCommand {
    fn name() -> &'static str {
        "inspect"
    }

    fn execute(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let token = args.next_required("対象を指定してください。")?;
        let idx = resolve_country_index(ctx.game(), token)?;
        print_country_details(ctx.game(), idx);
        Ok(())
    }
}

pub struct ShowCommand;

impl Command for ShowCommand {
    fn name() -> &'static str {
        "show"
    }

    fn execute(ctx: &mut Context<'_>, args: Args<'_>) -> Result<()> {
        InspectCommand::execute(ctx, args)
    }
}
pub struct SetCommand;

impl Command for SetCommand {
    fn name() -> &'static str {
        "set"
    }

    fn execute(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let token = args.next_required("対象を指定してください。")?;
        let idx = resolve_country_index(ctx.game(), token)?;
        let infra = parse_percentage(args.next(), "インフラ")?;
        let military = parse_percentage(args.next(), "軍事")?;
        let welfare = parse_percentage(args.next(), "福祉")?;
        let diplomacy = parse_percentage(args.next(), "外交")?;
        let debt_service = parse_percentage(args.next(), "債務返済")?;
        let administration = parse_percentage(args.next(), "行政費")?;
        let research = parse_percentage(args.next(), "研究開発")?;
        let ensure_core = match args.next() {
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
        let total = allocation.total_percentage();
        let country_name = ctx.game().countries()[idx].name.clone();
        ctx.game_mut().update_allocations(idx, allocation)?;
        println!(
            "{} の予算配分を更新しました (合計 {:.1}%)",
            country_name, total
        );
        Ok(())
    }
}
pub struct TickCommand;

impl Command for TickCommand {
    fn name() -> &'static str {
        "tick"
    }

    fn execute(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let token = args.next_required("進める分数を指定してください。")?;
        let minutes: f64 = token
            .parse()
            .map_err(|_| anyhow!("分数は数値で指定してください。"))?;
        let multiplier = ctx.game().time_multiplier();
        let reports = ctx.game_mut().tick_minutes(minutes)?;
        let mut stdout = io::stdout();
        print_reports(&mut stdout, minutes * multiplier, &reports)?;
        Ok(())
    }
}

pub struct SpeedCommand;

impl Command for SpeedCommand {
    fn name() -> &'static str {
        "speed"
    }

    fn execute(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let token = args.next_required("新しい時間倍率を指定してください。")?;
        let multiplier = parse_speed(token)?;
        ctx.game_mut().set_time_multiplier(multiplier)?;
        println!(
            "時間倍率 x{:.2} に設定しました。",
            ctx.game().time_multiplier()
        );
        Ok(())
    }
}
pub struct IndustryCommand;

impl Command for IndustryCommand {
    fn name() -> &'static str {
        "industry"
    }

    fn execute(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let sub = args
            .next_required("industry サブコマンドを指定してください (例: subsidize)。")?
            .to_ascii_lowercase();
        match sub.as_str() {
            "subsidize" => IndustrySubsidizeCommand::run(ctx, args),
            other => bail!("未知の industry サブコマンドです: {}", other),
        }
    }
}

struct IndustrySubsidizeCommand;

impl IndustrySubsidizeCommand {
    fn run(ctx: &mut Context<'_>, mut args: Args<'_>) -> Result<()> {
        let sector_token =
            args.next_required("セクターは category:key 形式または既知のキーで指定してください。")?;
        let percent_text = args.next_required("補助率(%)を指定してください。")?;
        let percent: f64 = percent_text
            .parse()
            .map_err(|_| anyhow!("補助率は数値で指定してください。"))?;
        let sector_id = ctx.game().sector_registry().resolve(sector_token)?;
        let overview = ctx
            .game_mut()
            .apply_industry_subsidy_by_id(&sector_id, percent)?;
        println!(
            "{} ({}:{}) に補助金 {:.1}% を設定しました。直近コスト {:.1} / 生産量 {:.1}",
            overview.name,
            sector_id.category,
            sector_id.key,
            overview.subsidy_percent,
            overview.last_cost,
            overview.last_output
        );
        Ok(())
    }
}
pub struct QuitCommand;

impl Command for QuitCommand {
    fn name() -> &'static str {
        "quit"
    }

    fn execute(_ctx: &mut Context<'_>, _args: Args<'_>) -> Result<()> {
        println!("シミュレーションを終了します。");
        process::exit(0);
    }
}

pub struct ExitCommand;

impl Command for ExitCommand {
    fn name() -> &'static str {
        "exit"
    }

    fn execute(ctx: &mut Context<'_>, args: Args<'_>) -> Result<()> {
        QuitCommand::execute(ctx, args)
    }
}
