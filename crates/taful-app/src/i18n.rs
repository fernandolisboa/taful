//! Localized strings (spec §15.5).
//!
//! Every user-facing string goes through Fluent. Sources live in
//! `i18n/<locale>/*.ftl` at the workspace root and are embedded in the
//! binary. en-US is the source locale; pt-BR is the default and the only
//! shipped locale in the first slice. A missing message falls back to en-US,
//! then to the message id itself, so a gap is visible but never crashes.

use std::fmt;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// A locale the app has strings for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    /// Brazilian Portuguese: the default and the shipped locale.
    PtBr,
    /// US English: the source locale, used in development.
    EnUs,
}

impl Locale {
    /// Every locale, source locale first.
    pub const ALL: [Locale; 2] = [Locale::EnUs, Locale::PtBr];
    /// The locale used when the user has not chosen one.
    pub const DEFAULT: Locale = Locale::PtBr;
    /// The locale every other locale must match key for key.
    pub const SOURCE: Locale = Locale::EnUs;

    /// BCP 47 tag, also the directory name under `i18n/`.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::PtBr => "pt-BR",
            Locale::EnUs => "en-US",
        }
    }

    /// Parses a BCP 47 tag, case-insensitively (`pt-br`, `en-US`, `en_US`).
    pub fn from_tag(tag: &str) -> Option<Locale> {
        let tag = tag.replace('_', "-");
        Locale::ALL
            .into_iter()
            .find(|l| l.tag().eq_ignore_ascii_case(&tag))
    }

    fn langid(self) -> LanguageIdentifier {
        self.tag().parse().expect("locale tags are valid BCP 47")
    }
}

/// One embedded `.ftl` file.
#[derive(Debug, Clone, Copy)]
pub struct Source {
    /// Locale the file belongs to.
    pub locale: Locale,
    /// File name under `i18n/<locale>/`.
    pub file: &'static str,
    /// File contents.
    pub text: &'static str,
}

macro_rules! sources {
    ($($file:literal),* $(,)?) => {
        &[$(
            Source {
                locale: Locale::EnUs,
                file: $file,
                text: include_str!(concat!("../../../i18n/en-US/", $file)),
            },
            Source {
                locale: Locale::PtBr,
                file: $file,
                text: include_str!(concat!("../../../i18n/pt-BR/", $file)),
            },
        )*]
    };
}

/// Every embedded Fluent file. Adding a file under `i18n/` without listing it
/// here fails a test.
pub const SOURCES: &[Source] = sources![
    "app.ftl",
    "calc.ftl",
    "glossary.ftl",
    "hand.ftl",
    "lessons.ftl",
    "review.ftl",
    "settings.ftl",
    "table.ftl",
    "theme.ftl",
    "tour.ftl",
];

/// A Fluent file failed to parse or clashed with another file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I18nError {
    /// `<locale>/<file>` of the offending source.
    pub source: String,
    /// What went wrong.
    pub message: String,
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.source, self.message)
    }
}

impl std::error::Error for I18nError {}

type Bundle = FluentBundle<FluentResource>;

/// Formats messages for one locale, with en-US as the fallback.
pub struct Localizer {
    locale: Locale,
    primary: Bundle,
    fallback: Option<Bundle>,
}

impl fmt::Debug for Localizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Localizer")
            .field("locale", &self.locale)
            .finish_non_exhaustive()
    }
}

impl Localizer {
    /// Builds a localizer from the embedded sources.
    pub fn new(locale: Locale) -> Result<Self, I18nError> {
        let primary = bundle(locale)?;
        let fallback = if locale == Locale::SOURCE {
            None
        } else {
            Some(bundle(Locale::SOURCE)?)
        };
        Ok(Self {
            locale,
            primary,
            fallback,
        })
    }

    /// The locale messages are formatted in.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// Whether `id` exists in this locale (without falling back).
    pub fn has(&self, id: &str) -> bool {
        self.primary.has_message(id)
    }

    /// Formats a message without arguments.
    pub fn tr(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// Formats a message with named arguments.
    pub fn tr_args<'a>(&self, id: &str, args: &[(&'a str, FluentValue<'a>)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.clone());
        }
        self.format(id, Some(&fluent_args))
    }

    /// Display name of an opponent id (`template::opponent_id`): a bot
    /// profile, the reference, or a mixed table's key (#29), which lists
    /// each live opponent's position and profile.
    pub fn opponent(&self, id: &str) -> String {
        match crate::mixed::parse_table_key(id) {
            Some(seats) => {
                let parts: Vec<String> = seats
                    .iter()
                    .map(|(pos, p)| {
                        let p = p.as_deref().unwrap_or(taful_core::profile::REFERENCE);
                        format!("{pos} {}", self.tr(&format!("bot-profile-{p}")))
                    })
                    .collect();
                self.tr_args("bot-table", &[("seats", parts.join(", ").into())])
            }
            None => self.tr(&format!("bot-profile-{id}")),
        }
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
        std::iter::once(&self.primary)
            .chain(self.fallback.as_ref())
            .find_map(|b| format_in(b, id, args))
            .unwrap_or_else(|| id.to_owned())
    }
}

fn format_in(bundle: &Bundle, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors);
    // A formatting error (e.g. a missing argument) still yields text with the
    // placeholder visible. Debug builds fail loudly; the variable-parity test
    // catches a translation that renames a variable.
    debug_assert!(errors.is_empty(), "formatting `{id}`: {errors:?}");
    Some(text.into_owned())
}

fn bundle(locale: Locale) -> Result<Bundle, I18nError> {
    let mut bundle = Bundle::new_concurrent(vec![locale.langid()]);
    // Unicode isolation marks (FSI/PDI) render as boxes in some fonts and add
    // nothing for left-to-right locales.
    bundle.set_use_isolating(false);
    for src in SOURCES.iter().filter(|s| s.locale == locale) {
        let name = format!("{}/{}", locale.tag(), src.file);
        let resource = parse(src)?;
        bundle.add_resource(resource).map_err(|errs| I18nError {
            source: name.clone(),
            message: format!("{errs:?}"),
        })?;
    }
    Ok(bundle)
}

/// Parses one source, returning parser errors as an [`I18nError`].
pub fn parse(src: &Source) -> Result<FluentResource, I18nError> {
    FluentResource::try_new(src.text.to_owned()).map_err(|(_, errs)| I18nError {
        source: format!("{}/{}", src.locale.tag(), src.file),
        message: format!("{errs:?}"),
    })
}

/// Message and term ids defined by one source, as `id` and `id.attribute`.
pub fn keys(src: &Source) -> Result<Vec<String>, I18nError> {
    use fluent_syntax::ast::Entry;
    let resource = parse(src)?;
    let mut keys = Vec::new();
    for entry in resource.entries() {
        let (prefix, id, attrs) = match entry {
            Entry::Message(m) => ("", m.id.name, &m.attributes),
            Entry::Term(t) => ("-", t.id.name, &t.attributes),
            _ => continue,
        };
        keys.push(format!("{prefix}{id}"));
        keys.extend(attrs.iter().map(|a| format!("{prefix}{id}.{}", a.id.name)));
    }
    Ok(keys)
}

/// Variables (`$name`) each message of one source uses, keyed like [`keys`].
pub fn variables(
    src: &Source,
) -> Result<std::collections::BTreeMap<String, std::collections::BTreeSet<String>>, I18nError> {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

    fn pattern(p: &Pattern<&str>, out: &mut std::collections::BTreeSet<String>) {
        for el in &p.elements {
            if let PatternElement::Placeable { expression } = el {
                expr(expression, out);
            }
        }
    }
    fn expr(e: &Expression<&str>, out: &mut std::collections::BTreeSet<String>) {
        match e {
            Expression::Inline(i) => inline(i, out),
            Expression::Select { selector, variants } => {
                inline(selector, out);
                for v in variants {
                    pattern(&v.value, out);
                }
            }
        }
    }
    fn inline(i: &InlineExpression<&str>, out: &mut std::collections::BTreeSet<String>) {
        match i {
            InlineExpression::VariableReference { id } => {
                out.insert(id.name.to_owned());
            }
            InlineExpression::Placeable { expression } => expr(expression, out),
            InlineExpression::FunctionReference { arguments, .. } => {
                for a in &arguments.positional {
                    inline(a, out);
                }
                for a in &arguments.named {
                    inline(&a.value, out);
                }
            }
            _ => {}
        }
    }

    let resource = parse(src)?;
    let mut map = std::collections::BTreeMap::new();
    for entry in resource.entries() {
        let (prefix, id, value, attrs) = match entry {
            Entry::Message(m) => ("", m.id.name, m.value.as_ref(), &m.attributes),
            Entry::Term(t) => ("-", t.id.name, Some(&t.value), &t.attributes),
            _ => continue,
        };
        let mut vars = std::collections::BTreeSet::new();
        if let Some(v) = value {
            pattern(v, &mut vars);
        }
        map.insert(format!("{prefix}{id}"), vars);
        for a in attrs {
            let mut vars = std::collections::BTreeSet::new();
            pattern(&a.value, &mut vars);
            map.insert(format!("{prefix}{id}.{}", a.id.name), vars);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;

    fn key_set(locale: Locale) -> BTreeSet<String> {
        SOURCES
            .iter()
            .filter(|s| s.locale == locale)
            .flat_map(|s| keys(s).unwrap())
            .collect()
    }

    #[test]
    fn every_source_parses() {
        for src in SOURCES {
            parse(src).unwrap_or_else(|e| panic!("{e}"));
        }
        for locale in Locale::ALL {
            Localizer::new(locale).unwrap();
        }
    }

    /// Spec §15.5: a key in en-US but not in pt-BR fails the build. The
    /// reverse (a stale key left in a translation) fails too.
    #[test]
    fn locales_have_the_same_keys() {
        let source = key_set(Locale::SOURCE);
        assert!(!source.is_empty());
        for locale in Locale::ALL {
            let other = key_set(locale);
            let missing: Vec<_> = source.difference(&other).collect();
            let extra: Vec<_> = other.difference(&source).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{}: missing {missing:?}, not in {}: {extra:?}",
                locale.tag(),
                Locale::SOURCE.tag(),
            );
        }
    }

    /// A translation must use the same `$variables` as the source, or the
    /// UI shows a raw placeholder.
    #[test]
    fn locales_use_the_same_variables() {
        let vars = |locale: Locale| {
            let mut all = std::collections::BTreeMap::new();
            for src in SOURCES.iter().filter(|s| s.locale == locale) {
                all.extend(variables(src).unwrap());
            }
            all
        };
        let source = vars(Locale::SOURCE);
        assert!(
            source.values().any(|v| !v.is_empty()),
            "scan found no variables"
        );
        for locale in Locale::ALL {
            let other = vars(locale);
            for (key, expected) in &source {
                assert_eq!(
                    other.get(key),
                    Some(expected),
                    "{}: `{key}` variables differ from {}",
                    locale.tag(),
                    Locale::SOURCE.tag()
                );
            }
        }
    }

    #[test]
    fn every_ftl_file_on_disk_is_embedded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../i18n");
        for locale in Locale::ALL {
            for entry in std::fs::read_dir(root.join(locale.tag())).unwrap() {
                let name = entry.unwrap().file_name().into_string().unwrap();
                if name.ends_with(".ftl") {
                    assert!(
                        SOURCES.iter().any(|s| s.locale == locale && s.file == name),
                        "{}/{name} is not listed in i18n::SOURCES",
                        locale.tag(),
                    );
                }
            }
        }
    }

    #[test]
    fn product_term_matches_the_constant() {
        for locale in Locale::ALL {
            let l = Localizer::new(locale).unwrap();
            assert_eq!(l.tr("app-name"), crate::PRODUCT_NAME, "{}", locale.tag());
        }
    }

    #[test]
    fn default_locale_is_pt_br() {
        let l = Localizer::new(Locale::DEFAULT).unwrap();
        assert_eq!(l.locale(), Locale::PtBr);
        assert_eq!(l.tr("nav-settings"), "Configurações");
    }

    #[test]
    fn unknown_id_falls_back_to_the_id() {
        let l = Localizer::new(Locale::PtBr).unwrap();
        assert_eq!(l.tr("no-such-message"), "no-such-message");
    }

    #[test]
    fn arguments_are_formatted() {
        let l = Localizer::new(Locale::PtBr).unwrap();
        let text = l.tr_args("screen-coming-in", &[("milestone", "M2".into())]);
        assert!(text.contains("M2"), "{text}");
    }

    #[test]
    fn tags_round_trip() {
        for locale in Locale::ALL {
            assert_eq!(Locale::from_tag(locale.tag()), Some(locale));
        }
        assert_eq!(Locale::from_tag("pt_br"), Some(Locale::PtBr));
        assert_eq!(Locale::from_tag("fr-FR"), None);
    }
}
