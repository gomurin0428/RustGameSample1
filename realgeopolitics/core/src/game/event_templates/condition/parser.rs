use anyhow::{Result, anyhow};

use crate::game::country::CountryState;
use crate::game::economy::CreditRating;

use super::ConditionEvaluator;

pub(crate) fn parse_condition(text: &str) -> Result<Box<dyn ConditionEvaluator>> {
    let expr = ConditionExpr::parse(text)?;
    Ok(Box::new(expr))
}

#[derive(Debug, Clone)]
enum ConditionExpr {
    And(Box<ConditionExpr>, Box<ConditionExpr>),
    Or(Box<ConditionExpr>, Box<ConditionExpr>),
    Comparison(Comparison),
}

impl ConditionExpr {
    fn parse(text: &str) -> Result<Self> {
        let tokens = tokenize(text)?;
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(expr)
    }
}

impl ConditionEvaluator for ConditionExpr {
    fn evaluate(&self, country: &CountryState) -> bool {
        match self {
            ConditionExpr::And(lhs, rhs) => lhs.evaluate(country) && rhs.evaluate(country),
            ConditionExpr::Or(lhs, rhs) => lhs.evaluate(country) || rhs.evaluate(country),
            ConditionExpr::Comparison(comparison) => comparison.evaluate(country),
        }
    }
}

#[derive(Debug, Clone)]
struct Comparison {
    metric: MetricKey,
    op: CompareOp,
    value: f64,
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::country::{BudgetAllocation, CountryState};
    use crate::game::economy::{FiscalAccount, TaxPolicy};

    fn sample_country() -> CountryState {
        CountryState::new(
            "Evalia".to_string(),
            "Republic".to_string(),
            12.0,
            600.0,
            55,
            45,
            48,
            70,
            FiscalAccount::new(300.0, CreditRating::A),
            TaxPolicy::default(),
            BudgetAllocation::default(),
        )
    }

    #[test]
    fn parse_condition_supports_and_or_grouping() {
        let evaluator = parse_condition("stability > 50 && (approval >= 45 || debt_ratio < 60)")
            .expect("condition should parse");
        let mut country = sample_country();
        assert!(evaluator.evaluate(&country));
        country.approval = 40;
        assert!(evaluator.evaluate(&country));
        country.stability = 40;
        assert!(!evaluator.evaluate(&country));
    }

    #[test]
    fn parse_condition_rejects_unknown_metric() {
        match parse_condition("unknown_metric > 0") {
            Ok(_) => panic!("should fail"),
            Err(err) => {
                assert!(format!("{}", err).contains("未知の条件メトリクス"));
            }
        }
    }
}
