//! The placeholder expression language (spec §12.2).
//!
//! ```text
//! {{ calc: pot_odds(pot=100, bet=50) | pct(1) }}
//! {{ spot: srp-btn-bb.freq("BTN", "bet33") | pct(0) }}
//! ```
//!
//! Only the whitelisted pure functions in [`FUNCTIONS`] exist. There are no
//! variables, loops or user code. Arguments are named, each a number or a
//! double-quoted string. A placeholder that fails to parse or evaluate is a
//! render error, and `content-check` fails on any render error.

use std::fmt;

use taful_core::cards::{Board, Hand};
use taful_core::{calc, equity, range::Range};

use crate::format::{self, Formatter};

/// One `{{ … }}` occurrence in a text.
#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    /// Byte range of the whole `{{ … }}` in the source text.
    pub span: std::ops::Range<usize>,
    /// The inside, whitespace-normalized: the snapshot key.
    pub source: String,
}

/// A parsed placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub call: Call,
    pub formatter: Option<Formatter>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// `calc: name(arg=value, …)`.
    Calc {
        name: String,
        args: Vec<(String, Arg)>,
    },
    /// `spot: <id>.<method>(values…)`, a reference-strategy citation.
    Spot {
        id: String,
        method: String,
        args: Vec<Arg>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Num(f64),
    Text(String),
}

/// What a call evaluates to.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Text(String),
}

/// A placeholder failed to parse or evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprError(pub String);

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExprError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ExprError> {
    Err(ExprError(msg.into()))
}

/// Answers `spot:` citations from the reference strategy. The library lands
/// in M3/M4; until then citations are render errors.
pub trait SpotProvider {
    fn cite(&self, id: &str, method: &str, args: &[Arg]) -> Result<Value, ExprError>;
}

/// Finds every `{{ … }}` in `text`. An unclosed `{{` is an error.
pub fn find_placeholders(text: &str) -> Result<Vec<Placeholder>, ExprError> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("{{").map(|i| from + i) {
        let Some(end) = text[start + 2..].find("}}").map(|i| start + 2 + i) else {
            return err(format!("unclosed `{{{{` at byte {start}"));
        };
        let inner = &text[start + 2..end];
        out.push(Placeholder {
            span: start..end + 2,
            source: normalize(inner),
        });
        from = end + 2;
    }
    Ok(out)
}

/// Collapses whitespace outside string literals and trims.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let (mut in_str, mut pending_space) = (false, false);
    for c in s.trim().chars() {
        if in_str {
            out.push(c);
            if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        // Spaces only matter between two word characters.
        if pending_space && out.ends_with(|p: char| p.is_alphanumeric()) && c.is_alphanumeric() {
            out.push(' ');
        }
        pending_space = false;
        if c == '"' {
            in_str = true;
        }
        out.push(c);
        // Canonical spacing after separators, so `a=1,b=2` and `a = 1, b = 2`
        // share a snapshot key.
        if c == ',' || c == ':' {
            out.push(' ');
        }
        if c == '|' {
            out.insert(out.len() - 1, ' ');
            out.push(' ');
        }
    }
    out
}

// ---- parser -------------------------------------------------------------------------------

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            s: src.as_bytes(),
            src,
            pos: 0,
        }
    }

    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.pos) == Some(&c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), ExprError> {
        if self.eat(c) {
            Ok(())
        } else {
            err(format!(
                "expected `{}` at column {} in `{}`",
                c as char,
                self.pos + 1,
                self.src
            ))
        }
    }

    /// `[a-z0-9_-]+` (ids may contain dashes; function names do not).
    fn ident(&mut self) -> Result<&'a str, ExprError> {
        self.ws();
        let start = self.pos;
        while self.pos < self.s.len()
            && (self.s[self.pos].is_ascii_alphanumeric() || matches!(self.s[self.pos], b'_' | b'-'))
        {
            self.pos += 1;
        }
        if start == self.pos {
            return err(format!(
                "expected a name at column {} in `{}`",
                start + 1,
                self.src
            ));
        }
        Ok(&self.src[start..self.pos])
    }

    fn arg(&mut self) -> Result<Arg, ExprError> {
        self.ws();
        match self.s.get(self.pos) {
            Some(b'"') => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.s.len() && self.s[self.pos] != b'"' {
                    self.pos += 1;
                }
                if self.pos == self.s.len() {
                    return err(format!("unclosed string in `{}`", self.src));
                }
                let text = self.src[start..self.pos].to_owned();
                self.pos += 1;
                Ok(Arg::Text(text))
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' || *c == b'.' => {
                let start = self.pos;
                self.pos += 1;
                while self.pos < self.s.len()
                    && (self.s[self.pos].is_ascii_digit() || self.s[self.pos] == b'.')
                {
                    self.pos += 1;
                }
                let t = &self.src[start..self.pos];
                t.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .map(Arg::Num)
                    .ok_or_else(|| ExprError(format!("bad number `{t}` in `{}`", self.src)))
            }
            _ => err(format!(
                "expected a number or a \"string\" at column {} in `{}`",
                self.pos + 1,
                self.src
            )),
        }
    }

    fn done(&mut self) -> bool {
        self.ws();
        self.pos == self.s.len()
    }
}

/// Parses the inside of a placeholder (without the braces).
pub fn parse(src: &str) -> Result<Expr, ExprError> {
    let mut p = Parser::new(src);
    let kind = p.ident()?;
    p.expect(b':')?;
    let call = match kind {
        "calc" => {
            let name = p.ident()?.to_owned();
            p.expect(b'(')?;
            let mut args = Vec::new();
            if !p.eat(b')') {
                loop {
                    let key = p.ident()?.to_owned();
                    p.expect(b'=')?;
                    let value = p.arg()?;
                    if args.iter().any(|(k, _)| *k == key) {
                        return err(format!("argument `{key}` given twice in `{src}`"));
                    }
                    args.push((key, value));
                    if p.eat(b')') {
                        break;
                    }
                    p.expect(b',')?;
                }
            }
            Call::Calc { name, args }
        }
        "spot" => {
            // `id.method(…)`: the id may contain dots? No: ids are [a-z0-9_-].
            let id = p.ident()?.to_owned();
            p.expect(b'.')?;
            let method = p.ident()?.to_owned();
            p.expect(b'(')?;
            let mut args = Vec::new();
            if !p.eat(b')') {
                loop {
                    args.push(p.arg()?);
                    if p.eat(b')') {
                        break;
                    }
                    p.expect(b',')?;
                }
            }
            Call::Spot { id, method, args }
        }
        other => return err(format!("unknown placeholder kind `{other}` (calc, spot)")),
    };
    let formatter = if p.eat(b'|') {
        let name = p.ident()?.to_owned();
        let arg = if p.eat(b'(') {
            let a = match p.arg()? {
                Arg::Num(n) if (0.0..=6.0).contains(&n) && n.fract() == 0.0 => n as u8,
                _ => return err(format!("formatter `{name}` takes 0–6 decimals in `{src}`")),
            };
            p.expect(b')')?;
            Some(a)
        } else {
            None
        };
        Some(Formatter::new(&name, arg)?)
    } else {
        None
    };
    if !p.done() {
        return err(format!(
            "unexpected text at column {} in `{src}`",
            p.pos + 1
        ));
    }
    Ok(Expr { call, formatter })
}

// ---- functions ---------------------------------------------------------------------------

/// Argument type of a whitelisted function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Num,
    Text,
}

/// A whitelisted function: name, named parameters (all required), body.
pub struct Function {
    pub name: &'static str,
    pub params: &'static [(&'static str, ArgType)],
    pub doc: &'static str,
    eval: fn(&Args<'_>) -> Result<Value, ExprError>,
}

/// Arguments after type checking.
pub struct Args<'a>(&'a [(String, Arg)]);

impl Args<'_> {
    fn num(&self, k: &str) -> f64 {
        match self.0.iter().find(|(n, _)| n == k) {
            Some((_, Arg::Num(x))) => *x,
            _ => unreachable!("type-checked"),
        }
    }
    fn text(&self, k: &str) -> &str {
        match self.0.iter().find(|(n, _)| n == k) {
            Some((_, Arg::Text(t))) => t,
            _ => unreachable!("type-checked"),
        }
    }
    fn count(&self, k: &str) -> Result<u32, ExprError> {
        let x = self.num(k);
        if x >= 0.0 && x.fract() == 0.0 && x <= 1e6 {
            Ok(x as u32)
        } else {
            err(format!("`{k}` must be a whole number ≥ 0, got {x}"))
        }
    }
    fn positive(&self, k: &str) -> Result<f64, ExprError> {
        let x = self.num(k);
        if x > 0.0 {
            Ok(x)
        } else {
            err(format!("`{k}` must be > 0, got {x}"))
        }
    }
    fn hand(&self, k: &str) -> Result<Hand, ExprError> {
        self.text(k)
            .parse()
            .map_err(|e| ExprError(format!("`{k}`: {e}")))
    }
    fn board(&self, k: &str) -> Result<Board, ExprError> {
        let t = self.text(k);
        if t.is_empty() {
            return Ok(Board::new(&[]).expect("empty board"));
        }
        t.parse().map_err(|e| ExprError(format!("`{k}`: {e}")))
    }
    fn range(&self, k: &str) -> Result<Range, ExprError> {
        self.text(k)
            .parse()
            .map_err(|e| ExprError(format!("`{k}`: {e:?}")))
    }
}

use ArgType::{Num, Text};

const fn f(
    name: &'static str,
    params: &'static [(&'static str, ArgType)],
    doc: &'static str,
    eval: fn(&Args<'_>) -> Result<Value, ExprError>,
) -> Function {
    Function {
        name,
        params,
        doc,
        eval,
    }
}

fn num(x: f64) -> Result<Value, ExprError> {
    if x.is_finite() {
        Ok(Value::Num(x))
    } else {
        err("result is not a finite number")
    }
}

fn unseen(cards: u32) -> Result<u32, ExprError> {
    match cards {
        1 => Ok(46),
        2 => Ok(47),
        _ => err("`cards` (cards to come) must be 1 or 2"),
    }
}

/// Every function a placeholder may call.
pub const FUNCTIONS: &[Function] = &[
    f(
        "pot_odds",
        &[("pot", Num), ("bet", Num)],
        "Equity needed to call `bet` into `pot` (pot before the bet).",
        |a| num(calc::pot_odds(a.positive("pot")?, a.positive("bet")?)),
    ),
    f(
        "mdf",
        &[("pot", Num), ("bet", Num)],
        "Minimum defense frequency vs `bet` into `pot`.",
        |a| num(calc::mdf(a.positive("pot")?, a.positive("bet")?)),
    ),
    f(
        "alpha",
        &[("pot", Num), ("bet", Num)],
        "Fold frequency a bluff of `bet` into `pot` needs to break even.",
        |a| num(calc::alpha(a.positive("pot")?, a.positive("bet")?)),
    ),
    f(
        "spr",
        &[("stack", Num), ("pot", Num)],
        "Stack-to-pot ratio.",
        |a| num(calc::spr(a.positive("stack")?, a.positive("pot")?)),
    ),
    f(
        "call_ev",
        &[("equity", Num), ("pot", Num), ("bet", Num)],
        "EV of calling `bet` into `pot` (pot before the bet) with `equity` (0–1), no future betting.",
        |a| {
            let e = a.num("equity");
            if !(0.0..=1.0).contains(&e) {
                return err("`equity` must be in 0..1");
            }
            let (pot, bet) = (a.positive("pot")?, a.positive("bet")?);
            num(e * (pot + 2.0 * bet) - bet)
        },
    ),
    f(
        "hit_probability",
        &[("outs", Num), ("cards", Num)],
        "Exact chance to hit one of `outs` with `cards` (1 or 2) cards to come.",
        |a| {
            let (o, c) = (a.count("outs")?, a.count("cards")?);
            let u = unseen(c)?;
            if o > u {
                return err("more outs than unseen cards");
            }
            num(calc::hit_probability(o, c, u))
        },
    ),
    f(
        "rule_of_2_and_4",
        &[("outs", Num), ("cards", Num)],
        "Rule-of-2-and-4 estimate with the >8-outs correction.",
        |a| {
            let (o, c) = (a.count("outs")?, a.count("cards")?);
            unseen(c)?;
            num(calc::rule_of_2_and_4(o, c))
        },
    ),
    f(
        "outs",
        &[("hero", Text), ("villain", Text), ("board", Text)],
        "Exact number of next cards that put `hero` ahead of `villain`.",
        |a| {
            let board = a.board("board")?;
            if !matches!(board.len(), 3 | 4) {
                return err("`board` must be a flop or a turn");
            }
            let (h, v) = (a.hand("hero")?, a.hand("villain")?);
            let dead = h.mask().union(v.mask()).union(board.mask());
            if dead.len() != 4 + board.len() {
                return err("hands and board share a card");
            }
            num(calc::outs(h, v, &board).len() as f64)
        },
    ),
    f(
        "equity_vs_hand",
        &[("hero", Text), ("villain", Text), ("board", Text)],
        "Exact all-in equity (0–1) of `hero` vs `villain` on `board` (\"\" preflop).",
        |a| {
            let e = equity::hand_vs_hand(a.hand("hero")?, a.hand("villain")?, &a.board("board")?)
                .map_err(|e| ExprError(e.to_string()))?;
            num(e.equity)
        },
    ),
    f(
        "equity_vs_range",
        &[("hero", Text), ("villain", Text), ("board", Text)],
        "Exact equity (0–1) of `hero` vs the range `villain` on a flop, turn or river.",
        |a| {
            let e =
                equity::hand_vs_range(a.hand("hero")?, &a.range("villain")?, &a.board("board")?)
                    .map_err(|e| ExprError(e.to_string()))?;
            num(e.equity)
        },
    ),
    f(
        "combos",
        &[("range", Text)],
        "Number of combos in a range (e.g. \"AKs\" = 4, \"QQ\" = 6, \"AKo\" = 12).",
        |a| num(a.range("range")?.combos()),
    ),
    f(
        "choose",
        &[("n", Num), ("k", Num)],
        "Binomial coefficient n choose k.",
        |a| {
            let (n, k) = (a.count("n")?, a.count("k")?);
            if k > n {
                return num(0.0);
            }
            let k = k.min(n - k);
            num((0..k).fold(1.0, |acc, i| acc * f64::from(n - i) / f64::from(i + 1)))
        },
    ),
    f(
        "mul",
        &[("a", Num), ("b", Num)],
        "a × b (counting: 3 aces × 4 kings).",
        |a| num(a.num("a") * a.num("b")),
    ),
    f(
        "min",
        &[("a", Num), ("b", Num)],
        "The smaller of a and b (effective stacks).",
        |a| num(a.num("a").min(a.num("b"))),
    ),
    f(
        "div",
        &[("a", Num), ("b", Num)],
        "a / b (unit changes: chips to big blinds).",
        |a| num(a.num("a") / a.positive("b")?),
    ),
    f(
        "cards",
        &[("c", Text)],
        "Cards as text, for the `cards` formatter (\"AhKh\").",
        |a| {
            let c = a.text("c");
            taful_core::cards::parse_cards(c).map_err(|e| ExprError(e.to_string()))?;
            Ok(Value::Text(c.to_owned()))
        },
    ),
];

fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS.iter().find(|f| f.name == name)
}

/// Evaluates a parsed expression.
pub fn eval(expr: &Expr, spots: Option<&dyn SpotProvider>) -> Result<Value, ExprError> {
    match &expr.call {
        Call::Calc { name, args } => {
            let Some(f) = function(name) else {
                return err(format!("unknown function `{name}`"));
            };
            for (k, v) in args {
                let Some((_, ty)) = f.params.iter().find(|(p, _)| p == k) else {
                    return err(format!("`{name}` has no argument `{k}`"));
                };
                let ok = matches!((ty, v), (Num, Arg::Num(_)) | (Text, Arg::Text(_)));
                if !ok {
                    let want = if *ty == Num {
                        "a number"
                    } else {
                        "a \"string\""
                    };
                    return err(format!("`{name}`: `{k}` must be {want}"));
                }
            }
            for (p, _) in f.params {
                if !args.iter().any(|(k, _)| k == p) {
                    return err(format!("`{name}` needs argument `{p}`"));
                }
            }
            (f.eval)(&Args(args))
        }
        Call::Spot { id, method, args } => match spots {
            Some(s) => s.cite(id, method, args),
            None => err(format!(
                "spot citation `{id}.{method}` needs the strategy library (M4)"
            )),
        },
    }
}

/// A rendered placeholder: display text and, for numbers, the value in the
/// formatter's unit (percentage points for `pct`), used by numeric quizzes.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub text: String,
    pub value: Option<f64>,
}

/// Parses, evaluates and formats one placeholder source for `locale`.
pub fn render(
    source: &str,
    locale: &str,
    spots: Option<&dyn SpotProvider>,
) -> Result<Rendered, ExprError> {
    let expr = parse(source)?;
    let value = eval(&expr, spots)?;
    format::apply(expr.formatter.as_ref(), &value, locale)
}

/// Replaces every placeholder in `text`. Errors are collected per
/// placeholder; the text keeps the raw placeholder where one failed.
pub fn substitute(
    text: &str,
    locale: &str,
    spots: Option<&dyn SpotProvider>,
) -> (String, Vec<(String, ExprError)>) {
    let placeholders = match find_placeholders(text) {
        Ok(p) => p,
        Err(e) => return (text.to_owned(), vec![(String::new(), e)]),
    };
    let mut out = String::with_capacity(text.len());
    let mut errors = Vec::new();
    let mut last = 0;
    for p in placeholders {
        out.push_str(&text[last..p.span.start]);
        match render(&p.source, locale, spots) {
            Ok(r) => out.push_str(&r.text),
            Err(e) => {
                out.push_str(&text[p.span.clone()]);
                errors.push((p.source, e));
            }
        }
        last = p.span.end;
    }
    out.push_str(&text[last..]);
    (out, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(src: &str) -> Result<String, ExprError> {
        render(src, "en-US", None).map(|r| r.text)
    }

    #[test]
    fn spec_example() {
        assert_eq!(
            r("calc: pot_odds(pot=100, bet=50) | pct(1)").unwrap(),
            "25.0%"
        );
        assert_eq!(
            render("calc: pot_odds(pot=100, bet=50) | pct(1)", "pt-BR", None)
                .unwrap()
                .text,
            "25,0%"
        );
        let v = render("calc: pot_odds(pot=100, bet=50) | pct(1)", "en-US", None).unwrap();
        assert_eq!(v.value, Some(25.0));
    }

    #[test]
    fn functions() {
        assert_eq!(r("calc: mdf(pot=100, bet=50) | pct(0)").unwrap(), "67%");
        assert_eq!(r("calc: alpha(pot=100, bet=100) | pct(0)").unwrap(), "50%");
        assert_eq!(r("calc: spr(stack=400, pot=100) | ratio").unwrap(), "4.0:1");
        assert_eq!(r("calc: combos(range=\"AKs\") | int").unwrap(), "4");
        assert_eq!(r("calc: combos(range=\"AKo\") | int").unwrap(), "12");
        assert_eq!(r("calc: combos(range=\"QQ\") | int").unwrap(), "6");
        assert_eq!(r("calc: choose(n=52, k=2) | int").unwrap(), "1,326");
        assert_eq!(
            render("calc: choose(n=52, k=2) | int", "pt-BR", None)
                .unwrap()
                .text,
            "1.326"
        );
        assert_eq!(
            r("calc: outs(hero=\"AhJh\", villain=\"KsKc\", board=\"Kh7h2d\") | int").unwrap(),
            "8"
        );
        assert_eq!(
            r("calc: hit_probability(outs=9, cards=2) | pct(1)").unwrap(),
            "35.0%"
        );
        assert_eq!(
            r("calc: rule_of_2_and_4(outs=15, cards=2) | pct(0)").unwrap(),
            "53%"
        );
        assert_eq!(
            r("calc: call_ev(equity=0.25, pot=100, bet=50) | bb(1)").unwrap(),
            "0.0 bb"
        );
        assert_eq!(r("calc: cards(c=\"AhKs\") | cards").unwrap(), "A♥ K♠");
        assert_eq!(r("calc: mul(a=3, b=4) | int").unwrap(), "12");
        assert_eq!(r("calc: div(a=300, b=2) | int").unwrap(), "150");
        assert!(r("calc: div(a=1, b=0)").is_err());
        let e = render(
            "calc: equity_vs_range(hero=\"AsKs\", villain=\"QQ\", board=\"2c7dJh\") | pct(1)",
            "en-US",
            None,
        )
        .unwrap();
        assert!((20.0..30.0).contains(&e.value.unwrap()), "{e:?}");
    }

    #[test]
    fn errors() {
        for bad in [
            "calc: nope(x=1)",
            "calc: pot_odds(pot=100)",
            "calc: pot_odds(pot=100, bet=50, x=1)",
            "calc: pot_odds(pot=\"100\", bet=50)",
            "calc: pot_odds(pot=100, pot=100)",
            "calc: pot_odds(pot=100, bet=50) | nope",
            "calc: pot_odds(pot=100, bet=50) | pct(9)",
            "calc: pot_odds(pot=0, bet=50)",
            "calc: pot_odds(pot=100, bet=50) extra",
            "calc pot_odds(pot=100, bet=50)",
            "loop: x()",
            "calc: hit_probability(outs=9, cards=3)",
            "calc: outs(hero=\"AhAh\", villain=\"KsKc\", board=\"Kh7h2d\")",
            "calc: cards(c=\"Zz\")",
            "spot: srp.freq(\"BTN\", \"bet33\")",
        ] {
            assert!(r(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn finding_and_substituting() {
        let text = "Need {{ calc: pot_odds(pot=100,bet=50) | pct(1) }} and {{calc: mdf(pot=100, bet=50)|pct(0)}}.";
        let ps = find_placeholders(text).unwrap();
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].source, "calc: pot_odds(pot=100, bet=50) | pct(1)");
        assert_eq!(ps[1].source, "calc: mdf(pot=100, bet=50) | pct(0)");
        let (out, errs) = substitute(text, "en-US", None);
        assert!(errs.is_empty());
        assert_eq!(out, "Need 25.0% and 67%.");
        assert!(find_placeholders("oops {{ calc: x(").is_err());
        let (out, errs) = substitute("a {{ calc: nope() }} b", "en-US", None);
        assert_eq!(errs.len(), 1);
        assert_eq!(out, "a {{ calc: nope() }} b");
    }

    #[test]
    fn normalization_keeps_strings() {
        assert_eq!(
            normalize(" calc:  combos( range = \"AKs, QQ\" )|int "),
            "calc: combos(range=\"AKs, QQ\") | int"
        );
    }

    #[test]
    fn spot_syntax_parses() {
        let e = parse("spot: srp-btn-bb.freq(\"BTN\", \"bet33\") | pct(0)").unwrap();
        assert_eq!(
            e.call,
            Call::Spot {
                id: "srp-btn-bb".into(),
                method: "freq".into(),
                args: vec![Arg::Text("BTN".into()), Arg::Text("bet33".into())],
            }
        );
        struct Fixed;
        impl SpotProvider for Fixed {
            fn cite(&self, _: &str, _: &str, _: &[Arg]) -> Result<Value, ExprError> {
                Ok(Value::Num(0.42))
            }
        }
        let out = render(
            "spot: srp-btn-bb.freq(\"BTN\", \"bet33\") | pct(0)",
            "en-US",
            Some(&Fixed),
        )
        .unwrap();
        assert_eq!(out.text, "42%");
    }
}
