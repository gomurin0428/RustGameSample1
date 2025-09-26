use anyhow::{Result, anyhow};
use serde::Deserialize;

use super::country::CountryState;
use super::economy::CreditRating;
use super::{MAX_METRIC, MAX_RESOURCES, MIN_METRIC, MIN_RESOURCES};

const BUILTIN_TEMPLATES: &[TemplateSource] = &[
    TemplateSource::Yaml(
        "debt_crisis.yaml",
        include_str!("../../../config/events/debt_crisis.yaml"),
    ),
    TemplateSource::Json(
        "resource_boom.json",
        include_str!("../../../config/events/resource_boom.json"),
    ),
];

pub(crate) fn load_event_templates(country_count: usize) -> Result<Vec<ScriptedEventState>> {
    BUILTIN_TEMPLATES
        .iter()
        .enumerate()
        .map(|(idx, source)| compile_template(idx, source, country_count))
        .collect()
}
#[derive(Debug, Clone, Copy)]
enum TemplateSource {
    Yaml(&'static str, &'static str),
    Json(&'static str, &'static str),
}

fn compile_template(
    source_index: usize,
    source: &TemplateSource,
    country_count: usize,
) -> Result<ScriptedEventState> {
    let raw: EventTemplateRaw = match source {
        TemplateSource::Yaml(name, body) => serde_yaml::from_str(body)
            .map_err(|err| anyhow!("YAML テンプレート {} の解析に失敗しました: {}", name, err))?,
        TemplateSource::Json(name, body) => serde_json::from_str(body)
            .map_err(|err| anyhow!("JSON テンプレート {} の解析に失敗しました: {}", name, err))?,
    };
    let compiled = CompiledEventTemplate::new(raw).map_err(|err| {
        anyhow!(
            "イベントテンプレート {} のコンパイルに失敗しました: {}",
            source_index,
            err
        )
    })?;
    Ok(ScriptedEventState::new(compiled, country_count))
}
#[derive(Debug, Deserialize)]
struct EventTemplateRaw {
    id: String,
    name: String,
    description: String,
    condition: String,
    #[serde(default = "EventTemplateRaw::default_check_minutes")]
    check_minutes: u64,
    #[serde(default)]
    initial_delay_minutes: u64,
    #[serde(default = "EventTemplateRaw::default_cooldown_minutes")]
    cooldown_minutes: u64,
    #[serde(default)]
    effects: Vec<EventEffectRaw>,
}

impl EventTemplateRaw {
    const fn default_check_minutes() -> u64 {
        120
    }

    const fn default_cooldown_minutes() -> u64 {
        720
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum EventEffectRaw {
    #[serde(rename = "adjust_metric")]
    AdjustMetric { metric: String, delta: f64 },
    #[serde(rename = "report")]
    Report { message: String },
}
#[derive(Debug)]
struct CompiledEventTemplate {
    id: String,
    name: String,
    description: String,
    check_minutes: u64,
    initial_delay_minutes: u64,
    cooldown_minutes: f64,
    condition: ConditionExpr,
    effects: Vec<CompiledEffect>,
}
impl CompiledEventTemplate {
    fn new(raw: EventTemplateRaw) -> Result<Self> {
        if raw.check_minutes == 0 {
            return Err(anyhow!("check_minutes は 1 以上である必要があります"));
        }
        let condition = ConditionExpr::parse(&raw.condition)?;
        let mut effects = Vec::with_capacity(raw.effects.len());
        for effect in raw.effects {
            effects.push(CompiledEffect::from_raw(effect)?);
        }
        Ok(Self {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            check_minutes: raw.check_minutes,
            initial_delay_minutes: raw.initial_delay_minutes,
            cooldown_minutes: raw.cooldown_minutes as f64,
            condition,
            effects,
        })
    }
}
#[derive(Debug)]
pub(crate) struct ScriptedEventState {
    template: CompiledEventTemplate,
    last_triggered: Vec<Option<f64>>,
}

impl ScriptedEventState {
    fn new(template: CompiledEventTemplate, country_count: usize) -> Self {
        Self {
            template,
            last_triggered: vec![None; country_count],
        }
    }

    pub(crate) fn check_minutes(&self) -> u64 {
        self.template.check_minutes
    }

    pub(crate) fn initial_delay_minutes(&self) -> u64 {
        self.template.initial_delay_minutes
    }

    pub(crate) fn id(&self) -> &str {
        &self.template.id
    }

    pub(crate) fn name(&self) -> &str {
        &self.template.name
    }

    pub(crate) fn description(&self) -> &str {
        &self.template.description
    }

    fn ensure_capacity(&mut self, country_count: usize) {
        if self.last_triggered.len() < country_count {
            self.last_triggered.resize(country_count, None);
        }
    }

    pub(crate) fn execute(
        &mut self,
        countries: &mut [CountryState],
        current_minutes: f64,
    ) -> Vec<String> {
        self.ensure_capacity(countries.len());
        let mut reports = Vec::new();
        for (idx, country) in countries.iter_mut().enumerate() {
            if !self.template.condition.evaluate(country) {
                continue;
            }
            if let Some(last) = self.last_triggered[idx] {
                if current_minutes - last < self.template.cooldown_minutes {
                    continue;
                }
            }
            let mut local_reports = self.template.apply_effects(country);
            reports.append(&mut local_reports);
            self.last_triggered[idx] = Some(current_minutes);
        }
        reports
    }
}
#[derive(Debug, Clone)]
enum CompiledEffect {
    AdjustMetric { metric: MetricField, delta: f64 },
    Report { message: String },
}

impl CompiledEffect {
    fn from_raw(raw: EventEffectRaw) -> Result<Self> {
        match raw {
            EventEffectRaw::AdjustMetric { metric, delta } => {
                let field = MetricField::from_str(&metric)?;
                Ok(Self::AdjustMetric {
                    metric: field,
                    delta,
                })
            }
            EventEffectRaw::Report { message } => Ok(Self::Report { message }),
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum MetricField {
    Stability,
    Approval,
    Military,
    Resources,
    Gdp,
    Debt,
    CashReserve,
}

impl MetricField {
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "stability" => Ok(Self::Stability),
            "approval" => Ok(Self::Approval),
            "military" => Ok(Self::Military),
            "resources" => Ok(Self::Resources),
            "gdp" => Ok(Self::Gdp),
            "debt" => Ok(Self::Debt),
            "cash_reserve" => Ok(Self::CashReserve),
            other => Err(anyhow!("未知のメトリクス '{}' が指定されました", other)),
        }
    }

    fn apply(&self, country: &mut CountryState, delta: f64) {
        match self {
            MetricField::Stability => {
                country.stability = clamp_metric_delta(country.stability, delta);
            }
            MetricField::Approval => {
                country.approval = clamp_metric_delta(country.approval, delta);
            }
            MetricField::Military => {
                country.military = clamp_metric_delta(country.military, delta);
            }
            MetricField::Resources => {
                country.resources = clamp_resource_delta(country.resources, delta);
            }
            MetricField::Gdp => {
                country.gdp = (country.gdp + delta).max(0.0);
            }
            MetricField::Debt => {
                country.fiscal_mut().add_debt(delta);
            }
            MetricField::CashReserve => {
                let updated = (country.fiscal.cash_reserve() + delta).max(0.0);
                country.fiscal_mut().set_cash_reserve(updated);
            }
        }
    }
}
fn clamp_metric_delta(base: i32, delta: f64) -> i32 {
    let candidate = (base as f64 + delta).round() as i32;
    candidate.clamp(MIN_METRIC, MAX_METRIC)
}

fn clamp_resource_delta(base: i32, delta: f64) -> i32 {
    let candidate = (base as f64 + delta).round() as i32;
    candidate.clamp(MIN_RESOURCES, MAX_RESOURCES)
}
impl CompiledEventTemplate {
    fn apply_effects(&self, country: &mut CountryState) -> Vec<String> {
        let mut reports = Vec::new();
        for effect in &self.effects {
            match effect {
                CompiledEffect::AdjustMetric { metric, delta } => {
                    metric.apply(country, *delta);
                }
                CompiledEffect::Report { message } => {
                    reports.push(format_message(message, country));
                }
            }
        }
        reports
    }
}
fn format_message(template: &str, country: &CountryState) -> String {
    template.replace("{country}", &country.name)
}
#[derive(Debug, Clone)]
enum ConditionExpr {
    And(Box<ConditionExpr>, Box<ConditionExpr>),
    Or(Box<ConditionExpr>, Box<ConditionExpr>),
    Comparison(Comparison),
}

#[derive(Debug, Clone)]
struct Comparison {
    metric: MetricKey,
    op: CompareOp,
    value: f64,
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}
#[derive(Debug, Clone, Copy)]
enum MetricKey {
    Stability,
    Approval,
    Military,
    Resources,
    Gdp,
    Debt,
    CashReserve,
    DebtRatio,
    InterestRate,
    CreditRatingTier,
}
#[derive(Debug, Clone)]
enum Token {
    Ident(String),
    Number(f64),
    And,
    Or,
    LParen,
    RParen,
    Operator(CompareOp),
}
impl ConditionExpr {
    fn parse(text: &str) -> Result<Self> {
        let tokens = tokenize(text)?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(expr)
    }

    fn evaluate(&self, country: &CountryState) -> bool {
        match self {
            ConditionExpr::And(lhs, rhs) => lhs.evaluate(country) && rhs.evaluate(country),
            ConditionExpr::Or(lhs, rhs) => lhs.evaluate(country) || rhs.evaluate(country),
            ConditionExpr::Comparison(comparison) => comparison.evaluate(country),
        }
    }
}
impl Comparison {
    fn evaluate(&self, country: &CountryState) -> bool {
        let left = self.metric.value(country);
        match self.op {
            CompareOp::Lt => left < self.value,
            CompareOp::Le => left <= self.value,
            CompareOp::Gt => left > self.value,
            CompareOp::Ge => left >= self.value,
            CompareOp::Eq => (left - self.value).abs() < f64::EPSILON,
            CompareOp::Ne => (left - self.value).abs() >= f64::EPSILON,
        }
    }
}
impl MetricKey {
    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "stability" => Ok(Self::Stability),
            "approval" => Ok(Self::Approval),
            "military" => Ok(Self::Military),
            "resources" => Ok(Self::Resources),
            "gdp" => Ok(Self::Gdp),
            "debt" => Ok(Self::Debt),
            "cash_reserve" => Ok(Self::CashReserve),
            "debt_ratio" => Ok(Self::DebtRatio),
            "interest_rate" => Ok(Self::InterestRate),
            "credit_rating_tier" => Ok(Self::CreditRatingTier),
            other => Err(anyhow!("未知の条件メトリクス '{}' が指定されました", other)),
        }
    }

    fn value(&self, country: &CountryState) -> f64 {
        match self {
            MetricKey::Stability => country.stability as f64,
            MetricKey::Approval => country.approval as f64,
            MetricKey::Military => country.military as f64,
            MetricKey::Resources => country.resources as f64,
            MetricKey::Gdp => country.gdp.max(0.0),
            MetricKey::Debt => country.fiscal.debt.max(0.0),
            MetricKey::CashReserve => country.fiscal.cash_reserve().max(0.0),
            MetricKey::DebtRatio => compute_debt_ratio(country),
            MetricKey::InterestRate => country.fiscal.interest_rate.max(0.0),
            MetricKey::CreditRatingTier => credit_rating_tier(country.fiscal.credit_rating),
        }
    }
}
fn compute_debt_ratio(country: &CountryState) -> f64 {
    let debt = country.fiscal.debt.max(0.0);
    let gdp = country.gdp.max(0.0);
    if gdp <= f64::EPSILON {
        if debt <= f64::EPSILON {
            0.0
        } else {
            f64::INFINITY
        }
    } else {
        (debt / gdp) * 100.0
    }
}

fn credit_rating_tier(rating: CreditRating) -> f64 {
    match rating {
        CreditRating::AAA => 9.0,
        CreditRating::AA => 8.0,
        CreditRating::A => 7.0,
        CreditRating::BBB => 6.0,
        CreditRating::BB => 5.0,
        CreditRating::B => 4.0,
        CreditRating::CCC => 3.0,
        CreditRating::CC => 2.0,
        CreditRating::C => 1.0,
        CreditRating::D => 0.0,
    }
}
fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '&' => {
                chars.next();
                if matches!(chars.peek(), Some('&')) {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    return Err(anyhow!("'&' は '&&' として使用してください"));
                }
            }
            '|' => {
                chars.next();
                if matches!(chars.peek(), Some('|')) {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    return Err(anyhow!("'|' は '||' として使用してください"));
                }
            }
            '<' | '>' | '=' | '!' => {
                let op = read_operator(&mut chars)?;
                tokens.push(Token::Operator(op));
            }
            '0'..='9' | '.' => {
                let number = read_number(&mut chars)?;
                tokens.push(Token::Number(number));
            }
            _ if is_ident_start(ch) => {
                let ident = read_ident(&mut chars);
                tokens.push(Token::Ident(ident));
            }
            _ => {
                return Err(anyhow!(
                    "条件式に解釈できない文字 '{}' が含まれています",
                    ch
                ));
            }
        }
    }
    Ok(tokens)
}
fn read_operator(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<CompareOp> {
    let first = chars.next().expect("operator の読み取りに失敗しました");
    let second = chars.peek().copied();
    match (first, second) {
        ('<', Some('=')) => {
            chars.next();
            Ok(CompareOp::Le)
        }
        ('<', _) => Ok(CompareOp::Lt),
        ('>', Some('=')) => {
            chars.next();
            Ok(CompareOp::Ge)
        }
        ('>', _) => Ok(CompareOp::Gt),
        ('=', Some('=')) => {
            chars.next();
            Ok(CompareOp::Eq)
        }
        ('!', Some('=')) => {
            chars.next();
            Ok(CompareOp::Ne)
        }
        ('=', _) => Err(anyhow!("'=' は '==' として使用してください")),
        ('!', _) => Err(anyhow!("'!' は '!=' として使用してください")),
        _ => Err(anyhow!("不正な演算子です")),
    }
}
fn read_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<f64> {
    let mut buffer = String::new();
    let mut allow_sign = true;
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            buffer.push(ch);
            chars.next();
            allow_sign = false;
        } else if ch == '.' {
            buffer.push(ch);
            chars.next();
            allow_sign = false;
        } else if ch == 'e' || ch == 'E' {
            buffer.push(ch);
            chars.next();
            allow_sign = true;
        } else if (ch == '+' || ch == '-') && allow_sign {
            buffer.push(ch);
            chars.next();
            allow_sign = false;
        } else {
            break;
        }
    }
    if buffer.is_empty() || buffer == "+" || buffer == "-" {
        return Err(anyhow!("数値の解析に失敗しました"));
    }
    let value: f64 = buffer
        .parse()
        .map_err(|err| anyhow!("数値 '{}' の解析に失敗しました: {}", buffer, err))?;
    if matches!(chars.peek(), Some('%')) {
        chars.next();
        Ok(value)
    } else {
        Ok(value)
    }
}
fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut ident = String::new();
    while let Some(&ch) = chars.peek() {
        if is_ident_part(ch) {
            ident.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    ident
}

const fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

const fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_expression(&mut self) -> Result<ConditionExpr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<ConditionExpr> {
        let mut expr = self.parse_and()?;
        while self.consume_token(TokenKind::Or) {
            let rhs = self.parse_and()?;
            expr = ConditionExpr::Or(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<ConditionExpr> {
        let mut expr = self.parse_primary()?;
        while self.consume_token(TokenKind::And) {
            let rhs = self.parse_primary()?;
            expr = ConditionExpr::And(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<ConditionExpr> {
        match self.peek() {
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect_token(TokenKind::RParen)?;
                Ok(expr)
            }
            Some(Token::Ident(_)) => self.parse_comparison(),
            Some(token) => Err(anyhow!("予期しないトークン {:?} が出現しました", token)),
            None => Err(anyhow!("条件式が途中で終了しました")),
        }
    }

    fn parse_comparison(&mut self) -> Result<ConditionExpr> {
        let ident = self.expect_ident()?;
        let metric = MetricKey::from_str(&ident)?;
        let op = self.expect_operator()?;
        let value = self.expect_number()?;
        Ok(ConditionExpr::Comparison(Comparison { metric, op, value }))
    }

    fn expect_end(&self) -> Result<()> {
        if self.pos == self.tokens.len() {
            Ok(())
        } else {
            Err(anyhow!("トークンが余っています"))
        }
    }

    fn consume_token(&mut self, kind: TokenKind) -> bool {
        if matches!(self.peek(), Some(token) if kind.matches(token)) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_token(&mut self, kind: TokenKind) -> Result<()> {
        if self.consume_token(kind) {
            Ok(())
        } else {
            Err(anyhow!("期待したトークンが見つかりませんでした"))
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::Ident(value)) => Ok(value),
            _ => Err(anyhow!("識別子が必要です")),
        }
    }

    fn expect_operator(&mut self) -> Result<CompareOp> {
        match self.advance() {
            Some(Token::Operator(op)) => Ok(op),
            _ => Err(anyhow!("比較演算子が必要です")),
        }
    }

    fn expect_number(&mut self) -> Result<f64> {
        match self.advance() {
            Some(Token::Number(value)) => Ok(value),
            _ => Err(anyhow!("数値が必要です")),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos >= self.tokens.len() {
            None
        } else {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TokenKind {
    And,
    Or,
    RParen,
}

impl TokenKind {
    fn matches(self, token: &Token) -> bool {
        match (self, token) {
            (TokenKind::And, Token::And) => true,
            (TokenKind::Or, Token::Or) => true,
            (TokenKind::RParen, Token::RParen) => true,
            _ => false,
        }
    }
}
