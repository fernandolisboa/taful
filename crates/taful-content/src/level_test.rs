//! Level tests (D24): one TOML file per level under `content/tests/`,
//! named after the level.
//!
//! ```toml
//! level = "fundamentos"
//! version = 1
//! pass = 0.8          # share of right answers needed
//! draw = 10           # questions asked per attempt
//! pool = [            # `<quiz id>/<question id>`, from this level's lessons
//!     "hand-rankings-quiz/flush-vs-straight",
//!     …
//! ]
//! ```
//!
//! An attempt draws `draw` questions from the pool spread over the level's
//! lessons: one question from each lesson in a random order, then a second
//! round, and so on. Passing a level's test opens the next level in the
//! Prova mode; it never marks the lessons done (D25).

use serde::{Deserialize, Serialize};

/// A level test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelTest {
    /// The catalog level it tests (and the file's name).
    pub level: String,
    pub version: u32,
    /// Score (0..1) needed to pass.
    #[serde(default = "default_pass")]
    pub pass: f64,
    /// Questions per attempt.
    pub draw: usize,
    /// `<quiz id>/<question id>` of every question it may ask.
    pub pool: Vec<String>,
}

fn default_pass() -> f64 {
    0.8
}

/// The folder level tests live in, relative to the content root.
pub const DIR: &str = "tests";

/// The file of a level's test, relative to the content root.
pub fn path(level: &str) -> String {
    format!("{DIR}/{level}.toml")
}

impl LevelTest {
    pub fn parse(text: &str) -> Result<LevelTest, String> {
        let t: LevelTest =
            toml::from_str(&text.replace("\r\n", "\n")).map_err(|e| e.to_string())?;
        if !(t.pass > 0.0 && t.pass <= 1.0) {
            return Err(format!("pass must be in (0, 1], got {}", t.pass));
        }
        if t.draw == 0 {
            return Err("draw must be at least 1".into());
        }
        Ok(t)
    }

    /// The pool as `(quiz id, question id)`; `None` for a malformed entry.
    pub fn refs(&self) -> impl Iterator<Item = (&str, Option<(&str, &str)>)> {
        self.pool.iter().map(|r| {
            (
                r.as_str(),
                r.split_once('/')
                    .filter(|(q, x)| !q.is_empty() && !x.is_empty()),
            )
        })
    }

    /// The questions to ask in one attempt, as indexes into
    /// [`LevelTest::pool`]. `group` says which lesson an entry belongs to
    /// (entries of one lesson share a key) and `pick(n)` returns a uniform
    /// index below `n`. Questions are spread over the groups: a round takes
    /// one from each group, in a random order, until `draw` are picked.
    pub fn draw_indexes<K: PartialEq>(
        &self,
        group: impl Fn(&str) -> K,
        mut pick: impl FnMut(usize) -> usize,
    ) -> Vec<usize> {
        // Each group's remaining entries, groups in pool order.
        let mut groups: Vec<(K, Vec<usize>)> = Vec::new();
        for (i, r) in self.pool.iter().enumerate() {
            let k = group(r);
            match groups.iter_mut().find(|(g, _)| *g == k) {
                Some((_, v)) => v.push(i),
                None => groups.push((k, vec![i])),
            }
        }
        let mut out = Vec::new();
        while out.len() < self.draw && groups.iter().any(|(_, v)| !v.is_empty()) {
            let mut round: Vec<usize> = (0..groups.len())
                .filter(|&g| !groups[g].1.is_empty())
                .collect();
            shuffle(&mut round, &mut pick);
            for g in round {
                if out.len() == self.draw {
                    break;
                }
                let v = &mut groups[g].1;
                out.push(v.swap_remove(pick(v.len())));
            }
        }
        out
    }
}

/// Fisher–Yates with the caller's random source.
fn shuffle<T>(v: &mut [T], pick: &mut impl FnMut(usize) -> usize) {
    for i in (1..v.len()).rev() {
        v.swap(i, pick(i + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(draw: usize, pool: &[&str]) -> LevelTest {
        LevelTest {
            level: "l".into(),
            version: 1,
            pass: 0.8,
            draw,
            pool: pool.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn parses_and_validates() {
        let ok = "level = \"l1\"\nversion = 1\ndraw = 2\npool = [\"a-quiz/q\", \"b-quiz/q\"]\n";
        let t = LevelTest::parse(ok).unwrap();
        assert_eq!(t.pass, 0.8);
        assert_eq!(
            t.refs().map(|(_, r)| r).collect::<Vec<_>>(),
            vec![Some(("a-quiz", "q")), Some(("b-quiz", "q"))]
        );
        assert!(LevelTest::parse(&ok.replace("draw = 2", "draw = 0")).is_err());
        assert!(LevelTest::parse(&format!("{ok}pass = 1.5\n")).is_err());
        assert!(LevelTest::parse(&format!("{ok}extra = 1\n")).is_err());
        assert_eq!(
            test(1, &["nope", "/q", "a/"])
                .refs()
                .map(|(_, r)| r)
                .collect::<Vec<_>>(),
            vec![None, None, None]
        );
    }

    #[test]
    fn a_draw_spreads_over_the_lessons() {
        let t = test(4, &["a/1", "a/2", "a/3", "b/1", "b/2", "c/1"]);
        let lesson = |r: &str| r.split('/').next().unwrap().to_owned();
        let mut seed = 7u64;
        let mut rng = move |n: usize| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n
        };
        for _ in 0..50 {
            let got = t.draw_indexes(lesson, &mut rng);
            assert_eq!(got.len(), 4);
            let mut uniq = got.clone();
            uniq.sort();
            uniq.dedup();
            assert_eq!(uniq.len(), 4, "no repeats");
            // The first round has one question from each of a, b and c.
            let first: Vec<String> = got[..3].iter().map(|&i| lesson(&t.pool[i])).collect();
            for l in ["a", "b", "c"] {
                assert!(first.iter().any(|x| x == l), "{first:?}");
            }
        }
        // A draw larger than the pool asks every question once.
        assert_eq!(
            test(9, &["a/1", "b/1"])
                .draw_indexes(lesson, &mut rng)
                .len(),
            2
        );
    }
}
