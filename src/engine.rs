//! Calculator state machine — pure logic, no UI, no unsafe, fully unit-tested.

use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    pub fn sym(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "−",
            Op::Mul => "×",
            Op::Div => "÷",
        }
    }

    pub fn apply(self, a: f64, b: f64) -> Result<f64, ErrKey> {
        let v = match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => {
                if b == 0.0 {
                    return Err(ErrKey::DivZero);
                }
                a / b
            }
        };
        if v.is_finite() {
            Ok(v)
        } else {
            Err(ErrKey::Overflow)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrKey {
    DivZero,
    Invalid,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Sqr,
    Sqrt,
    Recip,
}

/// Typed expression kept for the small line above the main display.
#[derive(Clone, Debug, PartialEq)]
pub enum LastExpr {
    Binary(f64, Op, f64),
    UnarySqr(f64),
    UnarySqrt(f64),
    UnaryRecip(f64),
}

/// History entries are stored as typed data so they can be re-rendered
/// in any language / locale.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HistKind {
    Binary(f64, Op, f64),
    UnarySqr(f64),
    UnarySqrt(f64),
    UnaryRecip(f64),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: u64,
    pub kind: HistKind,
    /// Result in canonical ASCII form (num_to_string).
    pub result: String,
    #[serde(skip, default = "fresh_instant")]
    pub at: Instant,
}

pub fn fresh_instant() -> Instant {
    Instant::now()
}

/// What the display should show.
#[derive(Clone, Debug, PartialEq)]
pub enum Disp {
    Value(f64),
    Entry,
    Error(ErrKey),
}

pub struct Engine {
    /// Current typed number, canonical ASCII ("", "-", "0.", "1234.5", "-8").
    entry: String,
    /// Pending left operand.
    acc: Option<f64>,
    /// Pending binary operator.
    op: Option<Op>,
    /// Last action produced a final value (result sits in `entry`).
    just_eval: bool,
    pub last_expr: Option<LastExpr>,
    pub error: Option<ErrKey>,
    pub history: Vec<HistoryItem>,
    next_id: u64,
    history_dirty: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            entry: String::new(),
            acc: None,
            op: None,
            just_eval: false,
            last_expr: None,
            error: None,
            history: Vec::new(),
            next_id: 1,
            history_dirty: false,
        }
    }

    // ---------- queries ----------

    pub fn entry_str(&self) -> &str {
        &self.entry
    }

    fn current(&self) -> Option<f64> {
        if !self.entry.is_empty() {
            self.entry.parse::<f64>().ok()
        } else {
            self.acc
        }
    }

    pub fn raw_value(&self) -> Option<f64> {
        if self.error.is_some() {
            None
        } else {
            self.current()
        }
    }

    pub fn pending(&self) -> Option<(f64, Op)> {
        if self.error.is_some() {
            return None;
        }
        match (self.op, self.acc) {
            (Some(o), Some(a)) => Some((a, o)),
            _ => None,
        }
    }

    pub fn display(&self) -> Disp {
        if let Some(e) = self.error {
            return Disp::Error(e);
        }
        if self.just_eval && !self.entry.is_empty() {
            if let Ok(v) = self.entry.parse::<f64>() {
                return Disp::Value(v);
            }
        }
        if !self.entry.is_empty() {
            return Disp::Entry;
        }
        Disp::Value(self.acc.unwrap_or(0.0))
    }

    // ---------- input ----------

    pub fn digit(&mut self, d: u8) {
        if self.error.is_some() {
            self.c();
        }
        if self.just_eval {
            self.entry.clear();
            self.acc = None;
            self.op = None;
            self.last_expr = None;
            self.just_eval = false;
        }
        if self.entry.chars().filter(|c| c.is_ascii_digit()).count() >= 16 {
            return;
        }
        if self.entry == "0" || self.entry == "-0" {
            if d == 0 {
                return;
            }
            self.entry.pop();
        }
        self.entry.push((b'0' + d) as char);
    }

    pub fn dot(&mut self) {
        if self.error.is_some() {
            self.c();
        }
        if self.just_eval {
            self.entry.clear();
            self.acc = None;
            self.op = None;
            self.last_expr = None;
            self.just_eval = false;
        }
        if self.entry.contains('.') {
            return;
        }
        if self.entry.is_empty() || self.entry == "-" {
            self.entry.push_str("0.");
        } else {
            self.entry.push('.');
        }
    }

    pub fn set_op(&mut self, o: Op) {
        if self.error.is_some() {
            return;
        }
        if self.just_eval {
            self.just_eval = false;
            self.last_expr = None;
        }
        if let (Some(po), Some(a)) = (self.op, self.acc) {
            if !self.entry.is_empty() {
                let b = self.entry.parse::<f64>().unwrap_or(0.0);
                match po.apply(a, b) {
                    Ok(v) => self.acc = Some(v),
                    Err(e) => {
                        self.fail(e);
                        return;
                    }
                }
            }
        } else {
            self.acc = Some(self.current().unwrap_or(0.0));
        }
        self.entry.clear();
        self.op = Some(o);
    }

    pub fn equals(&mut self) {
        if self.error.is_some() {
            return;
        }
        // Only evaluate when there is a pending operation; repeating = is a
        // no-op (idempotent). No "repeat last operation": current input and
        // the operation cache are kept strictly apart.
        if let (Some(po), Some(a)) = (self.op, self.acc) {
            let b = self.current().unwrap_or(a);
            match po.apply(a, b) {
                Ok(v) => {
                    self.record(LastExpr::Binary(a, po, b), v);
                    self.finish(v);
                }
                Err(e) => self.fail(e),
            }
        }
    }

    /// Percent = ordinary math: divide the current number by 100 (200 % → 2, 5 % → 0.05).
    /// Idempotent-safe: repeated presses keep dividing by 100; the result stays a
    /// valid plain number, never malformed text.
    pub fn percent(&mut self) {
        if self.error.is_some() {
            return;
        }
        let v = self.current().unwrap_or(0.0);
        let r = v / 100.0;
        if !r.is_finite() {
            self.fail(ErrKey::Overflow);
            return;
        }
        self.entry = num_to_string(r);
        self.just_eval = false;
        self.last_expr = None;
    }

    pub fn negate(&mut self) {
        if self.error.is_some() {
            return;
        }
        if self.entry.is_empty() {
            if let Some(a) = self.acc {
                self.entry = num_to_string(-a);
                self.just_eval = false;
            }
            return;
        }
        if let Some(rest) = self.entry.strip_prefix('-') {
            self.entry = rest.to_owned();
        } else {
            self.entry.insert(0, '-');
        }
        self.just_eval = false;
    }

    pub fn unary(&mut self, k: Unary) {
        if self.error.is_some() {
            return;
        }
        let v = self.current().unwrap_or(0.0);
        let r: Result<f64, ErrKey> = match k {
            Unary::Sqr => Op::Mul.apply(v, v),
            Unary::Sqrt => {
                if v < 0.0 {
                    Err(ErrKey::Invalid)
                } else {
                    Ok(v.sqrt())
                }
            }
            Unary::Recip => {
                if v == 0.0 {
                    Err(ErrKey::DivZero)
                } else {
                    Op::Div.apply(1.0, v)
                }
            }
        };
        match r {
            Ok(x) => {
                let expr = match k {
                    Unary::Sqr => LastExpr::UnarySqr(v),
                    Unary::Sqrt => LastExpr::UnarySqrt(v),
                    Unary::Recip => LastExpr::UnaryRecip(v),
                };
                self.record(expr, x);
                self.finish(x);
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn backspace(&mut self) {
        if self.error.is_some() {
            self.c();
            return;
        }
        if self.entry.is_empty() {
            return;
        }
        self.entry.pop();
        if self.entry == "-" {
            self.entry.clear();
        }
        self.just_eval = false;
        self.last_expr = None;
    }

    pub fn ce(&mut self) {
        if self.error.is_some() {
            self.c();
            return;
        }
        self.entry.clear();
        self.just_eval = false;
        self.last_expr = None;
    }

    pub fn c(&mut self) {
        self.entry.clear();
        self.acc = None;
        self.op = None;
        self.just_eval = false;
        self.last_expr = None;
        self.error = None;
    }

    /// Load a value (e.g. from the history panel) as the current entry.
    pub fn load_result(&mut self, s: &str) {
        if self.error.is_some() {
            self.c();
        }
        self.entry = s.to_owned();
        self.acc = None;
        self.op = None;
        self.just_eval = true;
        self.last_expr = None;
    }

    // ---------- history ----------

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_dirty = true;
    }

    pub fn set_history(&mut self, h: Vec<HistoryItem>) {
        let mut h = h;
        if h.len() > 50 {
            h.truncate(50);
        }
        self.next_id = h.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        self.history = h;
    }

    pub fn take_history_dirty(&mut self) -> bool {
        std::mem::take(&mut self.history_dirty)
    }

    // ---------- internals ----------

    fn finish(&mut self, v: f64) {
        self.entry = num_to_string(v);
        self.acc = None;
        self.op = None;
        self.just_eval = true;
    }

    fn fail(&mut self, e: ErrKey) {
        self.entry.clear();
        self.acc = None;
        self.op = None;
        self.just_eval = false;
        self.last_expr = None;
        self.error = Some(e);
    }

    fn record(&mut self, expr: LastExpr, v: f64) {
        let item = HistoryItem {
            id: self.next_id,
            kind: match &expr {
                LastExpr::Binary(a, o, b) => HistKind::Binary(*a, *o, *b),
                LastExpr::UnarySqr(x) => HistKind::UnarySqr(*x),
                LastExpr::UnarySqrt(x) => HistKind::UnarySqrt(*x),
                LastExpr::UnaryRecip(x) => HistKind::UnaryRecip(*x),
            },
            result: num_to_string(v),
            at: Instant::now(),
        };
        self.next_id += 1;
        self.history.insert(0, item);
        if self.history.len() > 50 {
            self.history.truncate(50);
        }
        self.last_expr = Some(expr);
        self.history_dirty = true;
    }
}

/// Robustness fuzz test: deterministic PRNG (xorshift) hammering the engine
/// with 200k random inputs, asserting the state machine never produces an
/// invalid string, a non-finite value without an error state, or an
/// unrecoverable state. Any failure returns Err with the step number.
pub fn fuzz_check() -> Result<(), String> {
    let mut st: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rnd = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        st
    };
    let mut c = Engine::new();

    let valid_entry = |e: &str| {
        let mut dots = 0;
        for (i, ch) in e.chars().enumerate() {
            match ch {
                '0'..='9' => {}
                '.' => {
                    dots += 1;
                    if dots > 1 {
                        return false;
                    }
                }
                '-' if i == 0 => {}
                _ => return false,
            }
        }
        true
    };

    for step in 0..200_000u64 {
        match rnd() % 12 {
            0..=5 => c.digit((rnd() % 10) as u8),
            6 => c.dot(),
            7 => c.set_op(match rnd() % 4 {
                0 => Op::Add,
                1 => Op::Sub,
                2 => Op::Mul,
                _ => Op::Div,
            }),
            8 => c.equals(),
            9 => c.percent(),
            10 => match rnd() % 3 {
                0 => c.unary(Unary::Sqr),
                1 => c.unary(Unary::Sqrt),
                _ => c.unary(Unary::Recip),
            },
            _ => {
                if rnd() % 40 == 0 {
                    c.c();
                } else {
                    c.backspace();
                }
            }
        }

        // Invariant 1: the entry string is always a valid canonical number
        if !valid_entry(c.entry_str()) {
            return Err(format!("fuzz step {step}: invalid entry {:?}", c.entry_str()));
        }
        // Invariant 2: the value is either finite or in an explicit error state
        if let Some(v) = c.raw_value() {
            if !v.is_finite() {
                return Err(format!("fuzz step {step}: non-finite value without error"));
            }
        }
        // Invariant 3: hard cap on history length
        if c.history.len() > 50 {
            return Err(format!("fuzz step {step}: history exceeded cap"));
        }
    }

    // Invariant 4: after hammering, C must fully reset; equals stays idempotent
    c.c();
    c.digit(2);
    c.set_op(Op::Add);
    c.digit(2);
    c.equals();
    let v = c.raw_value();
    c.equals();
    if c.raw_value() != v {
        return Err("fuzz: = is not idempotent after hammering".to_owned());
    }
    c.c();
    if c.display() != Disp::Value(0.0) {
        return Err("fuzz: C did not reset to 0".to_owned());
    }
    Ok(())
}

/// Canonical ASCII number → plain (never scientific) string.
///
/// entry / history always use plain number format (guarantees the entry state
/// machine stays valid); scientific notation exists only in the display layer
/// (i18n::fmt_value).
pub fn num_to_string(v: f64) -> String {
    if v == 0.0 {
        return "0".to_owned(); // also normalises -0.0
    }
    let mut s = format!("{v:.10}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_owned();
    }
    s
}

// ---------- verification ----------

pub fn self_check() -> Result<(), String> {
    fn eq<T: PartialEq + std::fmt::Debug>(a: T, b: T, what: &str) -> Result<(), String> {
        if a == b {
            Ok(())
        } else {
            Err(format!("{what}: {a:?} != {b:?}"))
        }
    }

    // 5 + 3 = 8, repeated = → 11
    let mut c = Engine::new();
    c.digit(5);
    c.set_op(Op::Add);
    c.digit(3);
    c.equals();
    eq(c.raw_value(), Some(8.0), "5+3")?;
    c.equals();
    eq(c.raw_value(), Some(8.0), "repeat = is a no-op")?;

    // operator replacement: 1 + × 3 = 3
    c.c();
    c.digit(1);
    c.set_op(Op::Add);
    c.set_op(Op::Mul);
    c.digit(3);
    c.equals();
    eq(c.raw_value(), Some(3.0), "op replace")?;

    // division by zero → error, then any digit starts over
    c.c();
    c.digit(5);
    c.set_op(Op::Div);
    c.digit(0);
    c.equals();
    eq(c.error, Some(ErrKey::DivZero), "5/0")?;
    c.digit(7);
    eq(c.error, None, "digit clears error")?;
    eq(c.raw_value(), Some(7.0), "fresh after error")?;

    // percent = ÷100 (pure math semantics)
    c.c();
    for d in [2u8, 0, 0] {
        c.digit(d);
    }
    c.percent();
    eq(c.entry_str(), "2", "200% → 2")?;
    c.percent();
    eq(c.entry_str(), "0.02", "200%% → 0.02")?;
    c.equals();
    eq(c.raw_value(), Some(0.02), "% then = keeps value")?;
    c.c();
    c.digit(5);
    c.percent();
    eq(c.entry_str(), "0.05", "5% → 0.05")?;

    // unary ops
    c.c();
    c.digit(9);
    c.unary(Unary::Sqrt);
    eq(c.raw_value(), Some(3.0), "√9")?;
    c.c();
    c.digit(9);
    c.negate();
    c.unary(Unary::Sqrt);
    eq(c.error, Some(ErrKey::Invalid), "√(-9)")?;
    c.c();
    c.digit(8);
    c.unary(Unary::Recip);
    eq(c.raw_value(), Some(0.125), "1/8")?;

    // entry editing
    c.c();
    c.digit(1);
    c.digit(2);
    c.digit(3);
    c.backspace();
    eq(c.entry_str(), "12", "backspace")?;
    c.c();
    c.digit(1);
    c.dot();
    c.digit(5);
    eq(c.entry_str(), "1.5", "decimal entry")?;
    c.c();
    c.digit(5);
    c.negate();
    eq(c.entry_str(), "-5", "negate")?;
    c.negate();
    eq(c.entry_str(), "5", "negate twice")?;

    // overflow via repeated squaring (1e8 → … → inf)
    c.c();
    for d in [1u8, 0, 0, 0, 0, 0, 0, 0] {
        c.digit(d);
    }
    for _ in 0..10 {
        c.unary(Unary::Sqr);
    }
    eq(c.error, Some(ErrKey::Overflow), "overflow")?;

    // history cap at 50
    c.c();
    for i in 0..60u32 {
        c.digit(1);
        c.set_op(Op::Add);
        c.digit((i % 9 + 1) as u8);
        c.equals();
    }
    eq(c.history.len(), 50, "history cap")?;
    c.clear_history();
    eq(c.history.len(), 0, "clear history")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_self_check() {
        super::self_check().expect("engine self check failed");
    }
}
