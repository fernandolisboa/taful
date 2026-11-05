//! Spot templates and leak buckets (spec §2, §13, D6).
//!
//! A **spot template** is a spot with the board replaced by its board family
//! (the flop's texture class): configuration + line + postflop history +
//! texture class + hero position. It is the unit of spaced repetition: a
//! graded decision below `Correct` creates or updates the review item of its
//! template, and a review deals a fresh spot of the same template.
//!
//! A **leak bucket** is coarser: (configuration, street, node type, texture
//! class). EV loss is aggregated per bucket to find leaks.

use std::fmt;

use taful_core::action::{Action, Line, Street};
use taful_core::config::GameConfig;
use taful_core::position::Position;

use crate::preflop::SpotKind;

/// The configuration id of preflop decisions (postflop decisions use the
/// library configuration id, e.g. `btn-bb-srp`).
pub const PREFLOP: &str = "preflop";

/// Template key format version.
const KEY_VERSION: &str = "v1";

/// What kind of decision a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Preflop(SpotKind),
    /// Postflop, no bet faced: check or bet.
    Unopened,
    /// Postflop, facing a bet.
    VsBet,
    /// Postflop, facing a raise.
    VsRaise,
}

impl NodeType {
    /// The postflop type for a node where `facing` bets/raises were made on
    /// the street.
    pub fn postflop(facing: usize) -> NodeType {
        match facing {
            0 => NodeType::Unopened,
            1 => NodeType::VsBet,
            _ => NodeType::VsRaise,
        }
    }

    /// Stable id, also the i18n key suffix (`node-type-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            NodeType::Preflop(k) => k.id(),
            NodeType::Unopened => "unopened",
            NodeType::VsBet => "vs-bet",
            NodeType::VsRaise => "vs-raise",
        }
    }

    pub fn parse(s: &str) -> Option<NodeType> {
        SpotKind::ALL
            .into_iter()
            .map(NodeType::Preflop)
            .chain([NodeType::Unopened, NodeType::VsBet, NodeType::VsRaise])
            .find(|t| t.id() == s)
    }

    pub const ALL_POSTFLOP: [NodeType; 3] =
        [NodeType::Unopened, NodeType::VsBet, NodeType::VsRaise];
}

/// Short form of a config key in template keys (64 bits of the hash).
pub fn short_config_key(config: &GameConfig) -> String {
    config.key().to_string()[..16].to_string()
}

/// Stable id of a street (`street-<id>` in i18n, storage).
pub fn street_id(s: Street) -> &'static str {
    match s {
        Street::Preflop => "preflop",
        Street::Flop => "flop",
        Street::Turn => "turn",
        Street::River => "river",
    }
}

pub fn parse_street(s: &str) -> Option<Street> {
    Street::ALL.into_iter().find(|&x| street_id(x) == s)
}

/// Fluent ids that describe a texture class id (`A-2t-u-1` → high card,
/// suits, pairing, connectedness), or `None` for an unknown id.
pub fn texture_label_keys(id: &str) -> Option<[String; 4]> {
    let parts: Vec<&str> = id.split('-').collect();
    // The high-card part may itself end in '-' (`9-`).
    let (high, rest) = match parts.as_slice() {
        ["9", "", rest @ ..] => ("9-", rest),
        [h, rest @ ..] => (*h, rest),
        [] => return None,
    };
    let [suits, paired, connected] = rest else {
        return None;
    };
    let ok = ["A", "K", "Q", "JT", "9-"].contains(&high)
        && ["r", "2t", "m"].contains(suits)
        && ["u", "p"].contains(paired)
        && ["0", "1", "2"].contains(connected);
    ok.then(|| {
        [
            format!("texture-high-{high}"),
            format!("texture-suits-{suits}"),
            format!("texture-paired-{paired}"),
            format!("texture-connected-{connected}"),
        ]
    })
}

/// A spot template (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpotTemplate {
    /// [`short_config_key`] of the game config.
    pub config: String,
    /// [`PREFLOP`] or a library configuration id.
    pub spot: String,
    /// Preflop: the line up to hero's decision. Postflop: empty (the
    /// configuration id names the line).
    pub line: Line,
    /// Postflop actions before hero's decision, one list per street from the
    /// flop (the last list is hero's street). Empty for preflop.
    pub postflop: Vec<Vec<(Position, Action)>>,
    /// Texture class id of the flop (postflop only).
    pub texture: Option<String>,
    pub hero: Position,
}

impl SpotTemplate {
    pub fn is_preflop(&self) -> bool {
        self.spot == PREFLOP
    }

    pub fn street(&self) -> Street {
        match (self.is_preflop(), self.postflop.len()) {
            (true, _) => Street::Preflop,
            (false, 0 | 1) => Street::Flop,
            (false, 2) => Street::Turn,
            _ => Street::River,
        }
    }

    /// The storage key (`review_items.item_key`).
    pub fn key(&self) -> String {
        let history = if self.is_preflop() {
            self.line.to_string()
        } else {
            self.postflop
                .iter()
                .map(|s| Line(s.clone()).to_string())
                .collect::<Vec<_>>()
                .join("/")
        };
        format!(
            "{KEY_VERSION}|{}|{}|{history}|{}|{}",
            self.config,
            self.spot,
            self.texture.as_deref().unwrap_or("-"),
            self.hero
        )
    }

    pub fn parse(key: &str) -> Option<SpotTemplate> {
        let parts: Vec<&str> = key.split('|').collect();
        let [v, config, spot, history, texture, hero] = parts[..] else {
            return None;
        };
        if v != KEY_VERSION || config.is_empty() || spot.is_empty() {
            return None;
        }
        let hero: Position = hero.parse().ok()?;
        let texture = (texture != "-").then(|| texture.to_string());
        let (line, postflop) = if spot == PREFLOP {
            if texture.is_some() {
                return None;
            }
            (history.parse::<Line>().ok()?, Vec::new())
        } else {
            texture.as_ref()?;
            let streets = history
                .split('/')
                .map(|s| s.parse::<Line>().map(|l| l.0))
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            if streets.len() > 3 {
                return None;
            }
            (Line::default(), streets)
        };
        Some(SpotTemplate {
            config: config.to_string(),
            spot: spot.to_string(),
            line,
            postflop,
            texture,
            hero,
        })
    }
}

impl fmt::Display for SpotTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key())
    }
}

/// Everything about a graded decision beyond the numbers: its template and
/// its leak bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotContext {
    pub template: SpotTemplate,
    pub street: Street,
    pub node_type: NodeType,
}

impl SpotContext {
    pub fn bucket(&self) -> LeakBucket {
        LeakBucket {
            spot: self.template.spot.clone(),
            street: self.street,
            node_type: self.node_type,
            texture: self.template.texture.clone(),
        }
    }
}

/// A leak bucket (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeakBucket {
    pub spot: String,
    pub street: Street,
    pub node_type: NodeType,
    pub texture: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_keys_round_trip() {
        let pre = SpotTemplate {
            config: "c1c6782d35338294".into(),
            spot: PREFLOP.into(),
            line: "UTG:f,HJ:r250".parse().unwrap(),
            postflop: vec![],
            texture: None,
            hero: Position::Co,
        };
        assert_eq!(pre.key(), "v1|c1c6782d35338294|preflop|UTG:f,HJ:r250|-|CO");
        assert_eq!(SpotTemplate::parse(&pre.key()), Some(pre.clone()));
        assert_eq!(pre.street(), Street::Preflop);

        let post = SpotTemplate {
            config: "c1c6782d35338294".into(),
            spot: "btn-bb-srp".into(),
            line: Line::default(),
            postflop: vec!["BB:x,BTN:b180,BB:c".parse::<Line>().unwrap().0, vec![]],
            texture: Some("A-2t-u-1".into()),
            hero: Position::Bb,
        };
        assert_eq!(
            post.key(),
            "v1|c1c6782d35338294|btn-bb-srp|BB:x,BTN:b180,BB:c/|A-2t-u-1|BB"
        );
        assert_eq!(SpotTemplate::parse(&post.key()), Some(post.clone()));
        assert_eq!(post.street(), Street::Turn);
        // A first flop decision has one empty street.
        let first = SpotTemplate {
            postflop: vec![vec![]],
            ..post
        };
        assert_eq!(SpotTemplate::parse(&first.key()), Some(first.clone()));
        assert_eq!(first.street(), Street::Flop);

        for bad in [
            "",
            "v2|a|preflop||-|CO",
            "v1|a|btn|x|-|BB",
            "v1|a|preflop||-|XX",
        ] {
            assert_eq!(SpotTemplate::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn dynamic_fluent_ids_exist() {
        // The UI builds these ids at run time, so the desktop source lint
        // cannot see them.
        let textures: Vec<String> = taful_core::iso::Flop::canonical_all()
            .into_iter()
            .map(|(f, _)| f.texture().class().id())
            .collect();
        for locale in crate::i18n::Locale::ALL {
            let i18n = crate::i18n::Localizer::new(locale).unwrap();
            for s in Street::ALL {
                assert!(i18n.has(&format!("street-{}", street_id(s))));
            }
            for t in NodeType::ALL_POSTFLOP
                .into_iter()
                .chain(SpotKind::ALL.map(NodeType::Preflop))
            {
                assert!(i18n.has(&format!("node-type-{}", t.id())), "{t:?}");
            }
            for id in &textures {
                for k in texture_label_keys(id).unwrap_or_else(|| panic!("{id}")) {
                    assert!(i18n.has(&k), "{k}");
                }
            }
            assert!(i18n.has(&format!("table-postflop-{PREFLOP}")));
        }
        assert_eq!(texture_label_keys("A-2t-x-1"), None);
    }

    #[test]
    fn node_types() {
        for t in NodeType::ALL_POSTFLOP
            .into_iter()
            .chain(SpotKind::ALL.map(NodeType::Preflop))
        {
            assert_eq!(NodeType::parse(t.id()), Some(t));
        }
        assert_eq!(NodeType::postflop(0), NodeType::Unopened);
        assert_eq!(NodeType::postflop(2), NodeType::VsRaise);
        for s in Street::ALL {
            assert_eq!(parse_street(street_id(s)), Some(s));
        }
    }
}
