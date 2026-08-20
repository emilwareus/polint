use serde_norway::Value as YamlValue;

pub fn parse_pnpm_workspace_packages(contents: &str) -> Vec<String> {
    let Ok(value) = serde_norway::from_str::<YamlValue>(contents) else {
        return Vec::new();
    };
    let Some(packages) = yaml_get(&value, "packages").and_then(YamlValue::as_sequence) else {
        return Vec::new();
    };
    let mut patterns = packages
        .iter()
        .filter_map(YamlValue::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    patterns.sort();
    patterns.dedup();
    patterns
}

fn yaml_get<'a>(value: &'a YamlValue, key: &str) -> Option<&'a YamlValue> {
    value.as_mapping()?.get(YamlValue::String(key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::parse_pnpm_workspace_packages;

    #[test]
    fn parse_pnpm_workspace_packages_reads_block_and_inline_sequences() {
        let patterns = parse_pnpm_workspace_packages(
            r#"
packages:
  - packages/*
  - "apps/*"
  - 'tools/*'
extra: true
"#,
        );

        assert_eq!(
            patterns,
            vec![
                "apps/*".to_string(),
                "packages/*".to_string(),
                "tools/*".to_string()
            ]
        );

        assert_eq!(
            parse_pnpm_workspace_packages(r#"packages: ["crates/*", "plugins/*"]"#),
            vec!["crates/*".to_string(), "plugins/*".to_string()]
        );
    }
}
