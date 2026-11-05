//! Mapping of graded results to FSRS ratings (spec §11, §13, D6).
//!
//! The scheduler itself (`fsrs` crate) lives in `taful-app`; core only decides
//! which rating an outcome earns, so every client rates the same way.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::grading::Grade;

/// FSRS review rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Rating {
    /// Graded decision: best → Easy, correct → Good, inaccuracy → Hard,
    /// mistake/blunder → Again.
    pub fn from_grade(g: Grade) -> Rating {
        match g {
            Grade::Best => Rating::Easy,
            Grade::Correct => Rating::Good,
            Grade::Inaccuracy => Rating::Hard,
            Grade::Mistake | Grade::Blunder => Rating::Again,
        }
    }

    /// Calc exercise: wrong → Again; right but over target → Hard; right
    /// within target → Good; right within half the target → Easy.
    pub fn from_calc(correct: bool, elapsed: Duration, target: Duration) -> Rating {
        if !correct {
            Rating::Again
        } else if elapsed > target {
            Rating::Hard
        } else if elapsed * 2 <= target {
            Rating::Easy
        } else {
            Rating::Good
        }
    }

    /// Quiz question: right → Good, wrong → Again.
    pub fn from_quiz(correct: bool) -> Rating {
        if correct { Rating::Good } else { Rating::Again }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings() {
        assert_eq!(Rating::from_grade(Grade::Best), Rating::Easy);
        assert_eq!(Rating::from_grade(Grade::Blunder), Rating::Again);
        let t = Duration::from_secs(10);
        let s = Duration::from_secs;
        assert_eq!(Rating::from_calc(false, s(1), t), Rating::Again);
        assert_eq!(Rating::from_calc(true, s(11), t), Rating::Hard);
        assert_eq!(Rating::from_calc(true, s(10), t), Rating::Good);
        assert_eq!(Rating::from_calc(true, s(6), t), Rating::Good);
        assert_eq!(Rating::from_calc(true, s(5), t), Rating::Easy);
        assert_eq!(Rating::from_quiz(true), Rating::Good);
    }
}
