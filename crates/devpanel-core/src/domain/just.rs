use serde::{Deserialize, Serialize};

use super::Platform;

/// One parameter of a just recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JustParam {
    pub name: String,
    /// Whether the recipe fails without a value: no default, not a `*`
    /// variadic.
    pub required: bool,
    /// A `*` or `+` variadic, which takes any number of values.
    pub variadic: bool,
}

/// A public recipe of a repository's Justfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JustRecipe {
    pub name: String,
    pub doc: Option<String>,
    pub parameters: Vec<JustParam>,
}

/// Whether `name` is a Justfile name in the repository root, in any case.
pub fn is_justfile(name: &str) -> bool {
    name.eq_ignore_ascii_case("justfile") || name.eq_ignore_ascii_case(".justfile")
}

/// The command that dumps a Justfile as JSON.
pub const JUST_DUMP: &str = "just --dump --dump-format json";

#[derive(Deserialize)]
struct Dump {
    recipes: std::collections::BTreeMap<String, RawRecipe>,
}

#[derive(Deserialize)]
struct RawRecipe {
    name: String,
    doc: Option<String>,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    parameters: Vec<RawParam>,
}

#[derive(Deserialize)]
struct RawParam {
    name: String,
    default: Option<serde_json::Value>,
    #[serde(default)]
    kind: String,
}

/// Reads the output of [`JUST_DUMP`]. Private recipes are left out and the
/// rest are sorted by name.
pub fn parse_just_dump(json: &str) -> Result<Vec<JustRecipe>, String> {
    let dump: Dump = serde_json::from_str(json).map_err(|error| format!("unreadable just output: {error}"))?;
    let mut recipes: Vec<JustRecipe> = dump
        .recipes
        .into_values()
        .filter(|r| !r.private && !r.name.starts_with('_'))
        .map(|r| JustRecipe {
            name: r.name,
            doc: r.doc.filter(|d| !d.trim().is_empty()),
            parameters: r
                .parameters
                .into_iter()
                .map(|p| JustParam {
                    required: p.default.is_none() && p.kind != "star",
                    variadic: p.kind == "star" || p.kind == "plus",
                    name: p.name,
                })
                .collect(),
        })
        .collect();
    recipes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(recipes)
}

/// Quotes one argument for the platform shell so it stays one argument.
pub fn quote_arg(arg: &str, platform: Platform) -> String {
    match platform {
        Platform::Unix => format!("'{}'", arg.replace('\'', "'\\''")),
        Platform::Windows => format!("\"{}\"", arg.replace('"', "\\\"")),
    }
}

/// `just <recipe> <args…>` for the platform shell. Trailing empty values are
/// dropped, since the recipe's defaults cover them; an empty value before a
/// filled one is refused, because just takes arguments by position.
pub fn just_command(recipe: &str, args: &[String], platform: Platform) -> Result<String, String> {
    if recipe.is_empty() || !recipe.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("not a recipe name: {recipe:?}"));
    }
    let used = args.iter().rposition(|a| !a.is_empty()).map_or(0, |last| last + 1);
    if let Some(gap) = args[..used].iter().position(String::is_empty) {
        return Err(format!("fill in value {} too: just takes arguments in order", gap + 1));
    }
    let mut command = format!("just {recipe}");
    for arg in &args[..used] {
        command.push(' ');
        command.push_str(&quote_arg(arg, platform));
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = r#"{"recipes":{
        "test":{"name":"test","doc":"Run the tests","private":false,"parameters":[]},
        "build":{"name":"build","doc":null,"private":false,"parameters":[
            {"name":"target","default":null,"kind":"singular"},
            {"name":"profile","default":{"x":1},"kind":"singular"},
            {"name":"rest","default":null,"kind":"star"}]},
        "_helper":{"name":"_helper","doc":null,"private":false,"parameters":[]},
        "hidden":{"name":"hidden","doc":null,"private":true,"parameters":[]}}}"#;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dump_lists_public_recipes_with_parameters() {
        let recipes = parse_just_dump(DUMP).unwrap();
        let names: Vec<_> = recipes.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["build", "test"]);
        assert_eq!(recipes[1].doc.as_deref(), Some("Run the tests"));
        let flags: Vec<_> = recipes[0].parameters.iter().map(|p| (p.required, p.variadic)).collect();
        assert_eq!(flags, [(true, false), (false, false), (false, true)]);
    }

    #[test]
    fn unreadable_output_is_an_error() {
        assert!(parse_just_dump("not json").is_err());
    }

    #[test]
    fn arguments_are_quoted_for_the_platform() {
        assert_eq!(just_command("run", &args(&["a b", "it's"]), Platform::Unix).unwrap(), r"just run 'a b' 'it'\''s'");
        assert_eq!(just_command("run", &args(&["a b", r#"say "hi""#]), Platform::Windows).unwrap(), r#"just run "a b" "say \"hi\"""#);
    }

    #[test]
    fn trailing_empty_values_are_dropped() {
        assert_eq!(just_command("test", &args(&["", ""]), Platform::Unix).unwrap(), "just test");
        assert_eq!(just_command("build", &args(&["x", ""]), Platform::Unix).unwrap(), "just build 'x'");
    }

    #[test]
    fn an_empty_value_before_a_filled_one_is_refused() {
        assert!(just_command("build", &args(&["", "x"]), Platform::Unix).unwrap_err().contains("value 1"));
    }

    #[test]
    fn odd_recipe_names_are_refused() {
        assert!(just_command("a; rm -rf /", &[], Platform::Unix).is_err());
        assert!(just_command("", &[], Platform::Unix).is_err());
    }

    #[test]
    fn justfile_names_match_in_any_case() {
        assert!(is_justfile("Justfile") && is_justfile(".justfile") && !is_justfile("justfile.bak"));
    }
}
