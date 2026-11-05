//! Architecture checks (spec §4, §17), run in CI:
//!
//! 1. **Dependency direction.** Each workspace crate may depend only on the
//!    workspace crates listed for it in [`RULES`]: `core ← solver ← app ←
//!    desktop`, `core ← content ← app`, `store ← app`. A new workspace member
//!    fails the check until it is added to [`RULES`].
//! 2. **No copyleft in shipped crates (D11).** The normal + build dependency
//!    graph of every shipped crate must not contain `postflop-solver` or any
//!    package licensed only under GPL/AGPL. Only `tools/library-build` may
//!    link the solver fork.
//!
//! Usage: `cargo run -p taful-arch-check` from anywhere in the workspace.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::process::{Command, ExitCode};

use serde_json::Value;

/// One workspace crate: the workspace crates it may depend on, and whether it
/// ships to users (and so must stay free of copyleft code).
struct Rule {
    name: &'static str,
    allowed: &'static [&'static str],
    shipped: bool,
}

const fn rule(name: &'static str, allowed: &'static [&'static str], shipped: bool) -> Rule {
    Rule {
        name,
        allowed,
        shipped,
    }
}

const RULES: &[Rule] = &[
    rule("taful-core", &[], true),
    rule("taful-solver", &["taful-core"], true),
    rule("taful-content", &["taful-core"], true),
    rule("taful-store", &[], true),
    rule(
        "taful-app",
        &["taful-core", "taful-solver", "taful-content", "taful-store"],
        true,
    ),
    rule("taful-desktop", &["taful-app"], true),
    rule(
        "taful-library-build",
        &["taful-core", "taful-solver"],
        false,
    ),
    rule("taful-content-check", &["taful-content"], false),
    rule("taful-arch-check", &[], false),
];

/// Packages no shipped crate may reach, whatever their license field says.
const FORBIDDEN_PACKAGES: &[&str] = &["postflop-solver"];

/// Third-party packages without an SPDX `license` field whose license file
/// was reviewed by hand and is not copyleft. Anything else without one fails.
const UNLICENSED_OK: &[&str] = &[];

fn main() -> ExitCode {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = match Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "arch-check: cargo metadata failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            );
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("arch-check: cannot run cargo metadata: {e}");
            return ExitCode::FAILURE;
        }
    };
    let metadata: Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("arch-check: cannot parse cargo metadata: {e}");
            return ExitCode::FAILURE;
        }
    };

    let violations = check(&metadata, RULES);
    if violations.is_empty() {
        println!("arch-check: ok ({} workspace crates)", RULES.len());
        ExitCode::SUCCESS
    } else {
        for v in &violations {
            eprintln!("arch-check: {v}");
        }
        ExitCode::FAILURE
    }
}

/// Runs both checks over `cargo metadata` output; returns the violations.
fn check(metadata: &Value, rules: &[Rule]) -> Vec<String> {
    let mut violations = Vec::new();
    let packages = packages(metadata);
    let members: BTreeMap<&str, &str> = metadata["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|id| Some((packages.get(id)?["name"].as_str()?, id)))
        .collect();

    // Every member has a rule and every rule names a member.
    for name in members.keys() {
        if !rules.iter().any(|r| r.name == *name) {
            violations.push(format!(
                "workspace crate `{name}` has no rule in tools/arch-check RULES"
            ));
        }
    }
    for r in rules {
        if !members.contains_key(r.name) {
            violations.push(format!("rule for `{}` names no workspace crate", r.name));
        }
    }

    // 1. Direct workspace dependencies (any kind, dev included) follow RULES.
    for r in rules {
        let Some(pkg) = members.get(r.name).and_then(|id| packages.get(id)) else {
            continue;
        };
        for dep in pkg["dependencies"].as_array().into_iter().flatten() {
            let Some(dep_name) = dep["name"].as_str() else {
                continue;
            };
            if members.contains_key(dep_name) && !r.allowed.contains(&dep_name) {
                violations.push(format!(
                    "`{}` must not depend on `{dep_name}` (allowed: {:?})",
                    r.name, r.allowed
                ));
            }
        }
    }

    // 2. Shipped graphs contain no forbidden or copyleft-only package.
    let graph = runtime_graph(metadata);
    for r in rules.iter().filter(|r| r.shipped) {
        let Some(root) = members.get(r.name) else {
            continue;
        };
        for id in reachable(&graph, root) {
            let Some(pkg) = packages.get(id) else {
                continue;
            };
            let name = pkg["name"].as_str().unwrap_or("?");
            let version = pkg["version"].as_str().unwrap_or("?");
            if FORBIDDEN_PACKAGES.contains(&name) {
                violations.push(format!(
                    "shipped crate `{}` reaches forbidden package `{name} {version}` (D11)",
                    r.name
                ));
            }
            let is_member = members.values().any(|m| *m == id);
            match pkg["license"].as_str() {
                Some(license) if is_copyleft_only(license) => violations.push(format!(
                    "shipped crate `{}` reaches `{name} {version}` licensed `{license}`",
                    r.name
                )),
                Some(_) => {}
                None if is_member || UNLICENSED_OK.contains(&name) => {}
                None => violations.push(format!(
                    "shipped crate `{}` reaches `{name} {version}` with no SPDX `license` \
                     field; review its license file and add it to UNLICENSED_OK",
                    r.name
                )),
            }
        }
    }

    violations
}

/// Package id → package object.
fn packages(metadata: &Value) -> BTreeMap<&str, &Value> {
    metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["id"].as_str()?, p)))
        .collect()
}

/// Package id → ids it depends on through normal or build edges (dev edges
/// do not ship).
fn runtime_graph(metadata: &Value) -> BTreeMap<&str, Vec<&str>> {
    let mut graph = BTreeMap::new();
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(id) = node["id"].as_str() else {
            continue;
        };
        let deps = node["deps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| {
                d["dep_kinds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|k| k["kind"].as_str() != Some("dev"))
            })
            .filter_map(|d| d["pkg"].as_str())
            .collect();
        graph.insert(id, deps);
    }
    graph
}

/// Every package reachable from `root`, excluding `root`.
fn reachable<'a>(graph: &BTreeMap<&'a str, Vec<&'a str>>, root: &'a str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([root]);
    while let Some(id) = queue.pop_front() {
        for &dep in graph.get(id).into_iter().flatten() {
            if seen.insert(dep) {
                queue.push_back(dep);
            }
        }
    }
    seen.remove(root);
    seen
}

/// Whether an SPDX expression (or legacy `A/B` form) can only be satisfied
/// with a GPL-family license: `OR` picks any branch, `AND` needs all,
/// parentheses group. `MIT OR GPL-3.0` is fine (we take MIT);
/// `(MIT OR Apache-2.0) AND GPL-3.0` is not. LGPL is not flagged. An
/// expression that does not parse counts as copyleft, so it gets a look.
fn is_copyleft_only(expr: &str) -> bool {
    let spaced = expr
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut pos = 0;
    match permissive_or(&tokens, &mut pos) {
        Some(ok) if pos == tokens.len() => !ok,
        _ => true,
    }
}

/// `expr := and ("OR" and)*`; returns whether a non-copyleft choice exists.
fn permissive_or(t: &[&str], pos: &mut usize) -> Option<bool> {
    let mut ok = permissive_and(t, pos)?;
    while t.get(*pos) == Some(&"OR") {
        *pos += 1;
        ok |= permissive_and(t, pos)?;
    }
    Some(ok)
}

/// `and := atom ("AND" atom)*`.
fn permissive_and(t: &[&str], pos: &mut usize) -> Option<bool> {
    let mut ok = permissive_atom(t, pos)?;
    while t.get(*pos) == Some(&"AND") {
        *pos += 1;
        ok &= permissive_atom(t, pos)?;
    }
    Some(ok)
}

/// `atom := "(" expr ")" | id ["WITH" exception]`.
fn permissive_atom(t: &[&str], pos: &mut usize) -> Option<bool> {
    let token = *t.get(*pos)?;
    *pos += 1;
    let ok = if token == "(" {
        let ok = permissive_or(t, pos)?;
        (t.get(*pos) == Some(&")")).then_some(())?;
        *pos += 1;
        ok
    } else if matches!(token, ")" | "OR" | "AND" | "WITH") {
        return None;
    } else {
        !(token.starts_with("GPL") || token.starts_with("AGPL"))
    };
    if t.get(*pos) == Some(&"WITH") {
        *pos += 2;
        t.get(*pos - 1)?;
    }
    Some(ok)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn pkg(name: &str, license: &str, deps: &[&str]) -> Value {
        json!({
            "id": format!("{name}-id"),
            "name": name,
            "version": "1.0.0",
            "license": license,
            "dependencies": deps.iter().map(|d| json!({ "name": d })).collect::<Vec<_>>(),
        })
    }

    fn node(name: &str, deps: &[(&str, Option<&str>)]) -> Value {
        json!({
            "id": format!("{name}-id"),
            "deps": deps.iter().map(|(d, kind)| json!({
                "pkg": format!("{d}-id"),
                "dep_kinds": [{ "kind": kind }],
            })).collect::<Vec<_>>(),
        })
    }

    const TEST_RULES: &[Rule] = &[
        rule("core", &[], true),
        rule("app", &["core"], true),
        rule("build", &["core"], false),
    ];

    fn metadata(packages: Vec<Value>, nodes: Vec<Value>, members: &[&str]) -> Value {
        json!({
            "packages": packages,
            "workspace_members": members.iter().map(|m| format!("{m}-id")).collect::<Vec<_>>(),
            "resolve": { "nodes": nodes },
        })
    }

    #[test]
    fn clean_workspace_passes() {
        let m = metadata(
            vec![
                pkg("core", "UNLICENSED", &[]),
                pkg("app", "UNLICENSED", &["core", "serde"]),
                pkg("build", "UNLICENSED", &["core", "postflop-solver"]),
                pkg("serde", "MIT OR Apache-2.0", &[]),
                pkg("postflop-solver", "AGPL-3.0-or-later", &[]),
            ],
            vec![
                node("core", &[]),
                node("app", &[("core", None), ("serde", None)]),
                node("build", &[("core", None), ("postflop-solver", None)]),
                node("serde", &[]),
                node("postflop-solver", &[]),
            ],
            &["core", "app", "build"],
        );
        assert_eq!(check(&m, TEST_RULES), Vec::<String>::new());
    }

    #[test]
    fn wrong_direction_and_agpl_are_caught() {
        let m = metadata(
            vec![
                pkg("core", "UNLICENSED", &["app"]),
                pkg("app", "UNLICENSED", &["core", "gplthing"]),
                pkg("build", "UNLICENSED", &[]),
                pkg("gplthing", "GPL-3.0-only", &["postflop-solver"]),
                pkg("postflop-solver", "AGPL-3.0-or-later", &[]),
            ],
            vec![
                node("core", &[("app", None)]),
                node("app", &[("core", None), ("gplthing", Some("build"))]),
                node("build", &[]),
                node("gplthing", &[("postflop-solver", None)]),
                node("postflop-solver", &[]),
            ],
            &["core", "app", "build"],
        );
        let v = check(&m, TEST_RULES);
        assert!(
            v.iter()
                .any(|s| s.contains("`core` must not depend on `app`")),
            "{v:?}"
        );
        assert!(
            v.iter()
                .any(|s| s.contains("forbidden package `postflop-solver")),
            "{v:?}"
        );
        assert!(
            v.iter().any(|s| s.contains("licensed `GPL-3.0-only`")),
            "{v:?}"
        );
    }

    #[test]
    fn dev_only_edges_do_not_ship() {
        let m = metadata(
            vec![
                pkg("core", "UNLICENSED", &[]),
                pkg("app", "UNLICENSED", &["core", "gplthing"]),
                pkg("build", "UNLICENSED", &[]),
                pkg("gplthing", "GPL-3.0-only", &[]),
            ],
            vec![
                node("core", &[]),
                node("app", &[("core", None), ("gplthing", Some("dev"))]),
                node("build", &[]),
                node("gplthing", &[]),
            ],
            &["core", "app", "build"],
        );
        assert_eq!(check(&m, TEST_RULES), Vec::<String>::new());
    }

    #[test]
    fn missing_license_field_is_caught() {
        let mut nolicense = pkg("mystery", "", &[]);
        nolicense["license"] = Value::Null;
        let m = metadata(
            vec![
                pkg("core", "UNLICENSED", &[]),
                pkg("app", "UNLICENSED", &["core", "mystery"]),
                pkg("build", "UNLICENSED", &[]),
                nolicense,
            ],
            vec![
                node("core", &[]),
                node("app", &[("core", None), ("mystery", None)]),
                node("build", &[]),
                node("mystery", &[]),
            ],
            &["core", "app", "build"],
        );
        let v = check(&m, TEST_RULES);
        assert!(
            v.iter().any(|s| s.contains("`mystery 1.0.0` with no SPDX")),
            "{v:?}"
        );
    }

    #[test]
    fn unknown_member_is_caught() {
        let m = metadata(
            vec![
                pkg("core", "UNLICENSED", &[]),
                pkg("extra", "UNLICENSED", &[]),
            ],
            vec![node("core", &[]), node("extra", &[])],
            &["core", "extra"],
        );
        let v = check(&m, TEST_RULES);
        assert!(v.iter().any(|s| s.contains("`extra` has no rule")), "{v:?}");
        assert!(
            v.iter().any(|s| s.contains("rule for `app` names no")),
            "{v:?}"
        );
    }

    #[test]
    fn license_expressions() {
        assert!(is_copyleft_only("AGPL-3.0-or-later"));
        assert!(is_copyleft_only("GPL-2.0+"));
        assert!(is_copyleft_only("GPL-3.0 AND MIT"));
        assert!(!is_copyleft_only("MIT OR GPL-3.0"));
        assert!(!is_copyleft_only("MIT/GPL-2.0"));
        assert!(!is_copyleft_only("LGPL-2.1-or-later"));
        assert!(!is_copyleft_only("Apache-2.0 WITH LLVM-exception"));
        assert!(!is_copyleft_only("(MIT OR Apache-2.0) AND Unicode-3.0"));
        assert!(is_copyleft_only("(MIT OR Apache-2.0) AND GPL-3.0-only"));
        assert!(!is_copyleft_only("(MIT OR GPL-3.0) AND Apache-2.0"));
        assert!(is_copyleft_only("MIT AND (GPL-2.0 OR AGPL-3.0)"));
        assert!(is_copyleft_only("MIT OR"), "malformed counts as copyleft");
        assert!(is_copyleft_only("(MIT"), "malformed counts as copyleft");
    }
}
