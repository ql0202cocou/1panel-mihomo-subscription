//! `mihomo`/`clash` -> `mihomo` conversion.
//!
//! Parses provider YAML, appends enabled custom nodes/groups, replaces `rules`
//! with the user's, and handles top-level keys per `docs/api-design.md`
//! (rule-providers passthrough, proxy-providers stripped, unknown keys
//! preserved). Validation follows `docs/api-design.md` and returns an itemized
//! error list for the generate modal.

use serde_yaml::{Mapping, Value};

use crate::yaml;

/// Built-in policy targets that are always valid in rules and group members.
const BUILTIN_POLICIES: &[&str] = &[
    "DIRECT",
    "REJECT",
    "REJECT-DROP",
    "PASS",
    "COMPATIBLE",
    "GLOBAL",
];

pub struct CustomNode {
    pub name: String,
    /// Full Mihomo proxy mapping as YAML text.
    pub content: String,
}

pub struct CustomGroup {
    pub name: String,
    pub group_type: String,
    pub members: Vec<String>,
    pub options: Option<serde_json::Value>,
}

pub struct ConvertInput<'a> {
    pub provider_yaml: &'a str,
    pub rules: &'a str,
    /// Enabled custom nodes only.
    pub nodes: Vec<CustomNode>,
    /// Enabled custom groups only.
    pub groups: Vec<CustomGroup>,
}

#[derive(Debug)]
pub enum ConvertError {
    /// Provider YAML could not be parsed (not a user-config validation issue).
    ProviderParse,
    /// Itemized validation failures, surfaced as `400` (see `api-design.md`).
    Validation(Vec<String>),
}

/// Convert provider YAML into a Mihomo config string.
pub fn convert(input: ConvertInput) -> Result<String, ConvertError> {
    let mut root =
        yaml::parse_mapping(input.provider_yaml).map_err(|_| ConvertError::ProviderParse)?;

    let provider_proxies = names_in(root.get("proxies"));
    let provider_groups = names_in(root.get("proxy-groups"));

    // Parse custom node content up front; collect parse failures as validation
    // errors rather than aborting.
    let mut errors: Vec<String> = Vec::new();
    let mut parsed_nodes: Vec<(String, Mapping)> = Vec::new();
    for node in &input.nodes {
        match yaml::parse_mapping(&node.content) {
            Ok(m) => parsed_nodes.push((node.name.clone(), m)),
            Err(_) => errors.push(format!(
                "custom node `{}` has invalid YAML content",
                node.name
            )),
        }
    }

    let custom_node_names: Vec<String> = input.nodes.iter().map(|n| n.name.clone()).collect();
    let custom_group_names: Vec<String> = input.groups.iter().map(|g| g.name.clone()).collect();

    validate(
        &input,
        &provider_proxies,
        &provider_groups,
        &custom_node_names,
        &custom_group_names,
        &mut errors,
    );

    if !errors.is_empty() {
        return Err(ConvertError::Validation(errors));
    }

    // ── Build the output config ──────────────────────────────────────────────

    // proxies: provider entries + enabled custom nodes.
    let mut proxies = sequence_of(root.get("proxies"));
    for (_, node) in parsed_nodes {
        proxies.push(Value::Mapping(node));
    }
    root.insert(Value::from("proxies"), Value::Sequence(proxies));

    // proxy-groups: provider entries + enabled custom groups.
    let mut groups = sequence_of(root.get("proxy-groups"));
    for group in input.groups {
        groups.push(Value::Mapping(build_group(group)));
    }
    root.insert(Value::from("proxy-groups"), Value::Sequence(groups));

    // rules: fully replaced with the user's rules.
    let rules: Vec<Value> = rule_lines(input.rules)
        .map(|(_, line)| Value::from(line))
        .collect();
    root.insert(Value::from("rules"), Value::Sequence(rules));

    // proxy-providers: stripped in the MVP (SSRF/caching bypass risk).
    root.remove(Value::from("proxy-providers"));

    // All other top-level keys (rule-providers, dns, tun, ...) pass through.

    serde_yaml::to_string(&Value::Mapping(root)).map_err(|_| ConvertError::ProviderParse)
}

fn validate(
    input: &ConvertInput,
    provider_proxies: &[String],
    provider_groups: &[String],
    custom_node_names: &[String],
    custom_group_names: &[String],
    errors: &mut Vec<String>,
) {
    // Custom group names must not collide with provider group names; custom
    // node names must not collide with provider proxy names (append-only MVP).
    for group in &input.groups {
        if provider_groups.contains(&group.name) {
            errors.push(format!(
                "custom group `{}` conflicts with a provider group name",
                group.name
            ));
        }
    }
    for name in custom_node_names {
        if provider_proxies.contains(name) {
            errors.push(format!(
                "custom node `{name}` conflicts with a provider proxy name"
            ));
        }
    }

    // Known reference targets: every proxy, every group, and built-in policies.
    let known = |name: &str| {
        provider_proxies.iter().any(|n| n == name)
            || provider_groups.iter().any(|n| n == name)
            || custom_node_names.iter().any(|n| n == name)
            || custom_group_names.iter().any(|n| n == name)
            || BUILTIN_POLICIES.contains(&name)
    };

    // Custom group members must reference something that exists.
    for group in &input.groups {
        for member in &group.members {
            if !known(member) {
                errors.push(format!(
                    "custom group `{}` references unknown member `{member}`",
                    group.name
                ));
            }
        }
    }

    // Each rule's policy target must exist. Advanced/logical rules we can't
    // parse reliably are passed through without target validation.
    for (lineno, line) in rule_lines(input.rules) {
        if let Some(target) = rule_target(line) {
            if !known(&target) {
                errors.push(format!(
                    "rules line {lineno} references unknown policy `{target}`"
                ));
            }
        }
    }
}

/// Extract proxy/group `name` values from a `proxies`/`proxy-groups` value.
fn names_in(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Clone a value's sequence, or an empty one if absent/not a sequence.
fn sequence_of(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Sequence(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn build_group(group: CustomGroup) -> Mapping {
    let mut m = Mapping::new();
    m.insert(Value::from("name"), Value::from(group.name));
    m.insert(Value::from("type"), Value::from(group.group_type));
    // Merge group-specific options (url, interval, ...) before the member list.
    if let Some(opts) = group.options {
        if let Ok(Value::Mapping(opt_map)) = serde_yaml::to_value(&opts) {
            for (k, v) in opt_map {
                m.insert(k, v);
            }
        }
    }
    let proxies = group.members.into_iter().map(Value::from).collect();
    m.insert(Value::from("proxies"), Value::Sequence(proxies));
    m
}

/// Iterate non-empty, non-comment rule lines with their 1-based line numbers
/// (numbered over the original text so messages match the editor).
fn rule_lines(rules: &str) -> impl Iterator<Item = (usize, &str)> {
    rules.lines().enumerate().filter_map(|(i, raw)| {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            None
        } else {
            Some((i + 1, line))
        }
    })
}

/// The policy target of a rule line, or `None` when it cannot be parsed
/// reliably (logical/nested rules) and should be passed through unchecked.
fn rule_target(line: &str) -> Option<String> {
    // Logical/nested rules contain parentheses; skip target validation.
    if line.contains('(') {
        return None;
    }
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    let kind = parts.first()?.to_ascii_uppercase();
    let target = if kind == "MATCH" {
        parts.get(1)
    } else {
        parts.get(2)
    };
    target.map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVIDER: &str = r#"
port: 7890
proxy-providers:
  remote:
    url: https://example.com/list
rule-providers:
  ads:
    type: http
    url: https://example.com/ads.yaml
dns:
  enable: true
proxies:
  - { name: hk-1, type: ss, server: 1.2.3.4, port: 8388 }
proxy-groups:
  - { name: Proxy, type: select, proxies: [hk-1] }
rules:
  - MATCH,DIRECT
"#;

    fn input<'a>(
        rules: &'a str,
        nodes: Vec<CustomNode>,
        groups: Vec<CustomGroup>,
    ) -> ConvertInput<'a> {
        ConvertInput {
            provider_yaml: PROVIDER,
            rules,
            nodes,
            groups,
        }
    }

    fn out(input: ConvertInput) -> Mapping {
        let yaml = convert(input).expect("conversion succeeds");
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[test]
    fn appends_nodes_and_groups_replaces_rules() {
        let nodes = vec![CustomNode {
            name: "my-ss".into(),
            content: "{ name: my-ss, type: ss, server: 9.9.9.9, port: 1080 }".into(),
        }];
        let groups = vec![CustomGroup {
            name: "MyGroup".into(),
            group_type: "select".into(),
            members: vec!["my-ss".into(), "hk-1".into(), "DIRECT".into()],
            options: None,
        }];
        let root = out(input(
            "DOMAIN-SUFFIX,example.com,MyGroup\nMATCH,Proxy",
            nodes,
            groups,
        ));

        let proxies = root.get("proxies").unwrap().as_sequence().unwrap();
        assert_eq!(names_in(root.get("proxies")), vec!["hk-1", "my-ss"]);
        assert_eq!(proxies.len(), 2);

        let groups = root.get("proxy-groups").unwrap().as_sequence().unwrap();
        assert_eq!(groups.len(), 2);

        let rules = root.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0].as_str().unwrap(),
            "DOMAIN-SUFFIX,example.com,MyGroup"
        );
    }

    #[test]
    fn strips_proxy_providers_and_passes_through_others() {
        let root = out(input("MATCH,DIRECT", vec![], vec![]));
        assert!(
            root.get("proxy-providers").is_none(),
            "proxy-providers stripped"
        );
        assert!(
            root.get("rule-providers").is_some(),
            "rule-providers passed through"
        );
        assert!(root.get("dns").is_some(), "unknown keys passed through");
        assert!(root.get("port").is_some());
    }

    #[test]
    fn group_name_collision_is_rejected() {
        let groups = vec![CustomGroup {
            name: "Proxy".into(), // collides with provider group
            group_type: "select".into(),
            members: vec!["DIRECT".into()],
            options: None,
        }];
        let err = convert(input("MATCH,DIRECT", vec![], groups)).unwrap_err();
        match err {
            ConvertError::Validation(errs) => {
                assert!(errs
                    .iter()
                    .any(|e| e.contains("conflicts with a provider group")));
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn rule_referencing_unknown_group_is_rejected() {
        let err = convert(input("DOMAIN,x.com,Ghost\nMATCH,DIRECT", vec![], vec![])).unwrap_err();
        match err {
            ConvertError::Validation(errs) => {
                assert_eq!(errs.len(), 1);
                assert!(errs[0].contains("rules line 1"));
                assert!(errs[0].contains("Ghost"));
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn group_member_referencing_unknown_is_rejected() {
        let groups = vec![CustomGroup {
            name: "G".into(),
            group_type: "select".into(),
            members: vec!["does-not-exist".into()],
            options: None,
        }];
        let err = convert(input("MATCH,DIRECT", vec![], groups)).unwrap_err();
        match err {
            ConvertError::Validation(errs) => {
                assert!(errs
                    .iter()
                    .any(|e| e.contains("unknown member `does-not-exist`")));
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn rules_can_target_provider_proxies_and_builtins() {
        // hk-1 is a provider proxy; DIRECT is a builtin — both valid targets.
        let root = out(input(
            "DOMAIN,a.com,hk-1\nIP-CIDR,1.2.3.4/32,DIRECT,no-resolve\nMATCH,Proxy",
            vec![],
            vec![],
        ));
        assert_eq!(root.get("rules").unwrap().as_sequence().unwrap().len(), 3);
    }

    #[test]
    fn logical_rules_pass_through_without_target_validation() {
        // Contains parentheses: target not parseable, must not error.
        let rules = "AND,((DOMAIN,x.com),(NETWORK,udp)),Proxy\nMATCH,DIRECT";
        assert!(convert(input(rules, vec![], vec![])).is_ok());
    }

    #[test]
    fn group_options_are_merged() {
        let groups = vec![CustomGroup {
            name: "Auto".into(),
            group_type: "url-test".into(),
            members: vec!["hk-1".into()],
            options: Some(serde_json::json!({"url": "http://x/generate_204", "interval": 300})),
        }];
        let root = out(input("MATCH,Auto", vec![], groups));
        let groups = root.get("proxy-groups").unwrap().as_sequence().unwrap();
        let auto = groups
            .iter()
            .find(|g| g.get("name").and_then(Value::as_str) == Some("Auto"))
            .unwrap();
        assert_eq!(auto.get("interval").unwrap().as_u64().unwrap(), 300);
        assert_eq!(auto.get("type").unwrap().as_str().unwrap(), "url-test");
    }

    #[test]
    fn invalid_provider_yaml_is_provider_parse_error() {
        let bad = ConvertInput {
            provider_yaml: "this: is: not: valid",
            rules: "MATCH,DIRECT",
            nodes: vec![],
            groups: vec![],
        };
        assert!(matches!(convert(bad), Err(ConvertError::ProviderParse)));
    }
}
