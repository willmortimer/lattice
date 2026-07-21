//! Read-time formula expression evaluation.
//!
//! Supports field refs `{column_name}`, numeric literals, `+ - * /`, parentheses,
//! and string concatenation with `&` (or `+` when both sides are text).

use std::collections::BTreeMap;
use std::fmt;

use crate::types::{CellValue, FormulaValue};

/// Error while parsing or validating a formula expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaError {
    message: String,
}

impl FormulaError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FormulaError {}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Field(String),
    Plus,
    Minus,
    Star,
    Slash,
    Amp,
    LParen,
    RParen,
}

/// Collect `{column}` references from a formula (deduplicated, ordered).
pub fn formula_field_refs(expression: &str) -> Result<Vec<String>, FormulaError> {
    let tokens = tokenize(expression)?;
    let mut refs = Vec::new();
    for token in tokens {
        if let Token::Field(name) = token {
            if !refs.iter().any(|existing| existing == &name) {
                refs.push(name);
            }
        }
    }
    Ok(refs)
}

/// Parse `expression` and ensure it is a complete formula (no trailing tokens).
pub fn validate_formula_syntax(expression: &str) -> Result<(), FormulaError> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return Err(FormulaError::new("formula expression must not be empty"));
    }
    let tokens = tokenize(trimmed)?;
    let mut parser = Parser::new(tokens);
    parser.parse_expr()?;
    if parser.peek().is_some() {
        return Err(FormulaError::new("unexpected trailing tokens in formula"));
    }
    Ok(())
}

/// Evaluate a formula against already-loaded row cell values.
///
/// Returns `Ok(None)` when any referenced cell is missing/null, or when an
/// arithmetic error occurs (for example division by zero). Parse errors return `Err`.
pub fn evaluate_formula(
    expression: &str,
    values: &BTreeMap<String, CellValue>,
) -> Result<Option<FormulaValue>, FormulaError> {
    validate_formula_syntax(expression)?;
    let tokens = tokenize(expression.trim())?;
    let mut parser = Parser::new(tokens);
    let result = parser.parse_expr_value(values)?;
    if parser.peek().is_some() {
        return Err(FormulaError::new("unexpected trailing tokens in formula"));
    }
    Ok(match result {
        EvalValue::Null => None,
        EvalValue::Number(n) => Some(FormulaValue::Number(n)),
        EvalValue::Text(text) => Some(FormulaValue::Text(text)),
    })
}

fn tokenize(input: &str) -> Result<Vec<Token>, FormulaError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            c if c.is_whitespace() => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '&' => {
                tokens.push(Token::Amp);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '{' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(FormulaError::new("unclosed field reference '{'"));
                }
                let name: String = chars[start..i].iter().collect();
                let name = name.trim();
                if name.is_empty() {
                    return Err(FormulaError::new("empty field reference {}"));
                }
                if !is_ident(name) {
                    return Err(FormulaError::new(format!(
                        "invalid field reference {{{name}}}"
                    )));
                }
                tokens.push(Token::Field(name.to_string()));
                i += 1; // skip '}'
            }
            '"' => {
                i += 1;
                let mut out = String::new();
                let mut closed = false;
                while i < chars.len() {
                    match chars[i] {
                        '"' => {
                            i += 1;
                            closed = true;
                            break;
                        }
                        '\\' if i + 1 < chars.len() => {
                            out.push(chars[i + 1]);
                            i += 2;
                        }
                        c => {
                            out.push(c);
                            i += 1;
                        }
                    }
                }
                if !closed {
                    return Err(FormulaError::new("unclosed string literal"));
                }
                tokens.push(Token::String(out));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let lit: String = chars[start..i].iter().collect();
                let number = lit.parse::<f64>().map_err(|_| {
                    FormulaError::new(format!("invalid numeric literal {lit:?}"))
                })?;
                tokens.push(Token::Number(number));
            }
            other => {
                return Err(FormulaError::new(format!(
                    "unexpected character {other:?} in formula"
                )));
            }
        }
    }
    Ok(tokens)
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, PartialEq)]
enum EvalValue {
    Null,
    Number(f64),
    Text(String),
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn bump(&mut self) -> Option<Token> {
        if self.index >= self.tokens.len() {
            return None;
        }
        let token = self.tokens[self.index].clone();
        self.index += 1;
        Some(token)
    }

    fn parse_expr(&mut self) -> Result<(), FormulaError> {
        self.parse_concat_syntax()?;
        Ok(())
    }

    fn parse_concat_syntax(&mut self) -> Result<(), FormulaError> {
        self.parse_sum_syntax()?;
        while matches!(self.peek(), Some(Token::Amp)) {
            self.bump();
            self.parse_sum_syntax()?;
        }
        Ok(())
    }

    fn parse_sum_syntax(&mut self) -> Result<(), FormulaError> {
        self.parse_product_syntax()?;
        while matches!(self.peek(), Some(Token::Plus | Token::Minus)) {
            self.bump();
            self.parse_product_syntax()?;
        }
        Ok(())
    }

    fn parse_product_syntax(&mut self) -> Result<(), FormulaError> {
        self.parse_unary_syntax()?;
        while matches!(self.peek(), Some(Token::Star | Token::Slash)) {
            self.bump();
            self.parse_unary_syntax()?;
        }
        Ok(())
    }

    fn parse_unary_syntax(&mut self) -> Result<(), FormulaError> {
        if matches!(self.peek(), Some(Token::Plus | Token::Minus)) {
            self.bump();
            return self.parse_unary_syntax();
        }
        self.parse_primary_syntax()
    }

    fn parse_primary_syntax(&mut self) -> Result<(), FormulaError> {
        match self.bump() {
            Some(Token::Number(_)) | Some(Token::String(_)) | Some(Token::Field(_)) => Ok(()),
            Some(Token::LParen) => {
                self.parse_concat_syntax()?;
                match self.bump() {
                    Some(Token::RParen) => Ok(()),
                    _ => Err(FormulaError::new("expected ')' in formula")),
                }
            }
            other => Err(FormulaError::new(format!(
                "expected value in formula, found {other:?}"
            ))),
        }
    }

    fn parse_expr_value(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        self.parse_concat(values)
    }

    fn parse_concat(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        let mut left = self.parse_sum(values)?;
        while matches!(self.peek(), Some(Token::Amp)) {
            self.bump();
            let right = self.parse_sum(values)?;
            left = concat_values(left, right);
        }
        Ok(left)
    }

    fn parse_sum(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        let mut left = self.parse_product(values)?;
        while let Some(op) = self.peek().cloned() {
            match op {
                Token::Plus => {
                    self.bump();
                    let right = self.parse_product(values)?;
                    left = add_or_concat(left, right);
                }
                Token::Minus => {
                    self.bump();
                    let right = self.parse_product(values)?;
                    left = arithmetic(left, right, |a, b| a - b);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_product(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        let mut left = self.parse_unary(values)?;
        while let Some(op) = self.peek().cloned() {
            match op {
                Token::Star => {
                    self.bump();
                    let right = self.parse_unary(values)?;
                    left = arithmetic(left, right, |a, b| a * b);
                }
                Token::Slash => {
                    self.bump();
                    let right = self.parse_unary(values)?;
                    left = match (left, right) {
                        (EvalValue::Null, _) | (_, EvalValue::Null) => EvalValue::Null,
                        (EvalValue::Number(_), EvalValue::Number(b)) if b == 0.0 => EvalValue::Null,
                        (EvalValue::Number(a), EvalValue::Number(b)) => EvalValue::Number(a / b),
                        _ => EvalValue::Null,
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        match self.peek() {
            Some(Token::Plus) => {
                self.bump();
                self.parse_unary(values)
            }
            Some(Token::Minus) => {
                self.bump();
                match self.parse_unary(values)? {
                    EvalValue::Null => Ok(EvalValue::Null),
                    EvalValue::Number(n) => Ok(EvalValue::Number(-n)),
                    EvalValue::Text(_) => Ok(EvalValue::Null),
                }
            }
            _ => self.parse_primary(values),
        }
    }

    fn parse_primary(
        &mut self,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<EvalValue, FormulaError> {
        match self.bump() {
            Some(Token::Number(n)) => Ok(EvalValue::Number(n)),
            Some(Token::String(text)) => Ok(EvalValue::Text(text)),
            Some(Token::Field(name)) => Ok(cell_to_eval(values.get(&name))),
            Some(Token::LParen) => {
                let value = self.parse_concat(values)?;
                match self.bump() {
                    Some(Token::RParen) => Ok(value),
                    _ => Err(FormulaError::new("expected ')' in formula")),
                }
            }
            other => Err(FormulaError::new(format!(
                "expected value in formula, found {other:?}"
            ))),
        }
    }
}

fn cell_to_eval(cell: Option<&CellValue>) -> EvalValue {
    match cell {
        None | Some(CellValue::Null) => EvalValue::Null,
        Some(CellValue::Integer(n)) => EvalValue::Number(*n as f64),
        Some(CellValue::Decimal(n)) => EvalValue::Number(*n),
        Some(CellValue::Boolean(flag)) => EvalValue::Number(if *flag { 1.0 } else { 0.0 }),
        Some(CellValue::Text(text) | CellValue::Date(text)) => {
            if text.is_empty() {
                EvalValue::Null
            } else if let Ok(n) = text.parse::<f64>() {
                EvalValue::Number(n)
            } else {
                EvalValue::Text(text.clone())
            }
        }
        Some(CellValue::Lookup { values }) => {
            if values.is_empty() {
                EvalValue::Null
            } else {
                EvalValue::Text(values.join(", "))
            }
        }
        Some(CellValue::Rollup { value: None }) => EvalValue::Null,
        Some(CellValue::Rollup { value: Some(n) }) => EvalValue::Number(*n),
        Some(CellValue::Relation { .. }) | Some(CellValue::Formula { .. }) => EvalValue::Null,
    }
}

fn concat_values(left: EvalValue, right: EvalValue) -> EvalValue {
    match (left, right) {
        (EvalValue::Null, _) | (_, EvalValue::Null) => EvalValue::Null,
        (left, right) => EvalValue::Text(format!("{}{}", display_eval(&left), display_eval(&right))),
    }
}

fn add_or_concat(left: EvalValue, right: EvalValue) -> EvalValue {
    match (&left, &right) {
        (EvalValue::Null, _) | (_, EvalValue::Null) => EvalValue::Null,
        (EvalValue::Number(a), EvalValue::Number(b)) => EvalValue::Number(a + b),
        (EvalValue::Text(_), EvalValue::Text(_))
        | (EvalValue::Text(_), EvalValue::Number(_))
        | (EvalValue::Number(_), EvalValue::Text(_)) => concat_values(left, right),
    }
}

fn arithmetic(
    left: EvalValue,
    right: EvalValue,
    op: impl Fn(f64, f64) -> f64,
) -> EvalValue {
    match (left, right) {
        (EvalValue::Null, _) | (_, EvalValue::Null) => EvalValue::Null,
        (EvalValue::Number(a), EvalValue::Number(b)) => EvalValue::Number(op(a, b)),
        _ => EvalValue::Null,
    }
}

fn display_eval(value: &EvalValue) -> String {
    match value {
        EvalValue::Null => String::new(),
        EvalValue::Number(n) => {
            if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                (*n as i64).to_string()
            } else {
                n.to_string()
            }
        }
        EvalValue::Text(text) => text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CellValue;
    use std::collections::BTreeMap;

    fn vals(pairs: &[(&str, CellValue)]) -> BTreeMap<String, CellValue> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn evaluates_arithmetic_and_field_refs() {
        let values = vals(&[
            ("price", CellValue::Decimal(12.5)),
            ("quantity", CellValue::Integer(2)),
        ]);
        assert_eq!(
            evaluate_formula("{price} * {quantity}", &values).unwrap(),
            Some(FormulaValue::Number(25.0))
        );
        assert_eq!(
            evaluate_formula("({price} + 1) * 2", &values).unwrap(),
            Some(FormulaValue::Number(27.0))
        );
    }

    #[test]
    fn missing_or_null_refs_yield_null() {
        let values = vals(&[("price", CellValue::Decimal(10.0))]);
        assert_eq!(
            evaluate_formula("{price} * {quantity}", &values).unwrap(),
            None
        );
        let values = vals(&[
            ("price", CellValue::Null),
            ("quantity", CellValue::Integer(2)),
        ]);
        assert_eq!(
            evaluate_formula("{price} * {quantity}", &values).unwrap(),
            None
        );
    }

    #[test]
    fn concatenates_with_amp_and_plus() {
        let values = vals(&[
            ("first", CellValue::Text("Ada".into())),
            ("last", CellValue::Text("Lovelace".into())),
        ]);
        assert_eq!(
            evaluate_formula(r#"{first} & " " & {last}"#, &values).unwrap(),
            Some(FormulaValue::Text("Ada Lovelace".into()))
        );
        assert_eq!(
            evaluate_formula("{first} + {last}", &values).unwrap(),
            Some(FormulaValue::Text("AdaLovelace".into()))
        );
    }

    #[test]
    fn extracts_field_refs() {
        assert_eq!(
            formula_field_refs("{price} * {quantity} + {price}").unwrap(),
            vec!["price".to_string(), "quantity".to_string()]
        );
    }

    #[test]
    fn rejects_bad_syntax() {
        assert!(validate_formula_syntax("").is_err());
        assert!(validate_formula_syntax("{price} *").is_err());
        assert!(validate_formula_syntax("2 +").is_err());
    }

    #[test]
    fn division_by_zero_is_null() {
        let values = vals(&[("a", CellValue::Integer(1)), ("b", CellValue::Integer(0))]);
        assert_eq!(evaluate_formula("{a} / {b}", &values).unwrap(), None);
    }
}
