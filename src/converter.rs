//! `mihomo`/`clash` -> `mihomo` conversion.
//!
//! Parses provider YAML, appends enabled custom nodes/groups, replaces `rules`
//! with the user's, and handles top-level keys per `docs/api-design.md`
//! (rule-providers passthrough + custom 规则集 merged on top, proxy-providers
//! stripped, unknown keys preserved). Validation follows `docs/api-design.md`
//! and returns an itemized
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

pub struct RuleProvider {
    pub name: String,
    /// `http` / `file` / `inline`.
    pub provider_type: String,
    /// `domain` / `ipcidr` / `classical`.
    pub behavior: String,
    /// Remaining keys (url, path, payload, format, interval, ...) as a JSON
    /// object; merged verbatim into the emitted mapping.
    pub options: Option<serde_json::Value>,
}

pub struct ConvertInput<'a> {
    pub provider_yaml: &'a str,
    pub rules: &'a str,
    /// Enabled custom nodes only.
    pub nodes: Vec<CustomNode>,
    /// Enabled custom groups only.
    pub groups: Vec<CustomGroup>,
    /// Enabled custom rule-providers (规则集) only. Merged into the output
    /// `rule-providers:` map on top of the provider's (custom overrides by name).
    pub rule_providers: Vec<RuleProvider>,
    /// Manual ordering of the **custom** nodes within the custom block (custom
    /// node names). Names present here are emitted first in this order; any not
    /// listed (newly added) keep their default relative position at the end. The
    /// provider block's internal order is always upstream (not user-orderable).
    pub node_order: Vec<String>,
    /// Order of the two node blocks in the output `proxies`: a permutation of
    /// `"provider"` / `"custom"`. Empty means the default `["provider","custom"]`.
    pub node_section_order: Vec<String>,
    /// Manual proxy-group ordering (group names). Names present here are emitted
    /// first in this order; any not listed keep their default position at the end.
    pub group_order: Vec<String>,
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
        &custom_node_names,
        &custom_group_names,
        &mut errors,
    );

    if !errors.is_empty() {
        return Err(ConvertError::Validation(errors));
    }

    // ── Build the output config ──────────────────────────────────────────────

    // proxies: two blocks concatenated by `node_section_order`. The provider
    // block keeps upstream order (not user-orderable); the custom block is the
    // enabled custom nodes reordered by `node_order` (new ones fall to the end).
    let provider_block = sequence_of(root.get("proxies"));
    let mut custom_block: Vec<Value> = parsed_nodes
        .into_iter()
        .map(|(_, node)| Value::Mapping(node))
        .collect();
    reorder_by_name(
        &mut custom_block,
        |item| item.get("name").and_then(Value::as_str),
        &input.node_order,
    );
    let proxies = concat_sections(provider_block, custom_block, &input.node_section_order);
    root.insert(Value::from("proxies"), Value::Sequence(proxies));

    // proxy-groups: fully replaced with the user's custom groups (like `rules`).
    // Provider groups are NOT passed through — the admin imports them as editable
    // custom groups via `import-provider-groups`, so a provider update never
    // changes groups unless re-imported. Apply the manual ordering after.
    let mut groups: Vec<Value> = input
        .groups
        .into_iter()
        .map(|group| Value::Mapping(build_group(group)))
        .collect();
    reorder_by_name(
        &mut groups,
        |item| item.get("name").and_then(Value::as_str),
        &input.group_order,
    );
    root.insert(Value::from("proxy-groups"), Value::Sequence(groups));

    // rules: fully replaced with the user's rules.
    let rules: Vec<Value> = rule_lines(input.rules)
        .map(|(_, line)| Value::from(line))
        .collect();
    root.insert(Value::from("rules"), Value::Sequence(rules));

    // proxy-providers: stripped in the MVP (SSRF/caching bypass risk).
    root.remove(Value::from("proxy-providers"));

    // rule-providers: the provider's pass through; custom 规则集 are merged on top
    // (a custom entry overrides a provider entry of the same name). Additive so
    // imported provider RULE-SET rules keep resolving.
    if !input.rule_providers.is_empty() {
        let mut map = match root.remove(Value::from("rule-providers")) {
            Some(Value::Mapping(m)) => m,
            _ => Mapping::new(),
        };
        for rp in input.rule_providers {
            let name = rp.name.clone();
            map.insert(Value::from(name), Value::Mapping(build_rule_provider(rp)));
        }
        root.insert(Value::from("rule-providers"), Value::Mapping(map));
    }

    // All other top-level keys (dns, tun, ...) pass through.

    serde_yaml::to_string(&Value::Mapping(root)).map_err(|_| ConvertError::ProviderParse)
}

fn validate(
    input: &ConvertInput,
    provider_proxies: &[String],
    custom_node_names: &[String],
    custom_group_names: &[String],
    errors: &mut Vec<String>,
) {
    // Custom node names must not collide with provider proxy names (proxies are
    // still passed through + appended). Provider groups are no longer in the
    // output (replaced by custom groups), so there is no group-name collision.
    for name in custom_node_names {
        if provider_proxies.contains(name) {
            errors.push(format!(
                "custom node `{name}` conflicts with a provider proxy name"
            ));
        }
    }

    // Known reference targets: provider proxies (passed through), custom nodes
    // (appended), custom groups (the only groups in the output), and built-in
    // policies. A reference to an un-imported provider group is unknown.
    let known = |name: &str| {
        provider_proxies.iter().any(|n| n == name)
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

/// Stable reorder of named items by a desired name order.
///
/// Items whose name appears in `order` are moved to the front in `order`'s
/// sequence; all others keep their original relative order and follow. Items
/// with no resolvable name, and `order` entries not present in `items`, are
/// ignored. When `order` is empty this is a no-op. Shared by the converter
/// (proxy mappings) and the preview endpoint (`ProxyPreview`s).
pub fn reorder_by_name<T, F>(items: &mut Vec<T>, name_of: F, order: &[String])
where
    F: Fn(&T) -> Option<&str>,
{
    if order.is_empty() || items.len() < 2 {
        return;
    }
    let rank: std::collections::HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    // Stable by (rank, original index); unranked items sort after ranked ones
    // while preserving their default order.
    let mut indexed: Vec<(usize, T)> = std::mem::take(items).into_iter().enumerate().collect();
    indexed.sort_by_key(|(idx, item)| {
        let r = name_of(item).and_then(|n| rank.get(n)).copied();
        (r.unwrap_or(usize::MAX), *idx)
    });
    *items = indexed.into_iter().map(|(_, item)| item).collect();
}

/// Concatenate the provider and custom node blocks per `section_order` (a
/// permutation of `"provider"`/`"custom"`). Defaults to provider-first when the
/// order is empty or doesn't name `"custom"` before `"provider"`.
pub fn concat_sections(
    provider: Vec<Value>,
    custom: Vec<Value>,
    section_order: &[String],
) -> Vec<Value> {
    if section_order.first().map(String::as_str) == Some("custom") {
        let mut out = custom;
        out.extend(provider);
        out
    } else {
        let mut out = provider;
        out.extend(custom);
        out
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

/// Build a rule-provider value (`{type, behavior, ...options}`). The `name` is
/// the map key, so it is not repeated inside the value.
fn build_rule_provider(rp: RuleProvider) -> Mapping {
    let mut m = Mapping::new();
    m.insert(Value::from("type"), Value::from(rp.provider_type));
    m.insert(Value::from("behavior"), Value::from(rp.behavior));
    if let Some(opts) = rp.options {
        if let Ok(Value::Mapping(opt_map)) = serde_yaml::to_value(&opts) {
            for (k, v) in opt_map {
                // Never let options shadow the typed columns.
                if matches!(k.as_str(), Some("type" | "behavior" | "name")) {
                    continue;
                }
                m.insert(k, v);
            }
        }
    }
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
            rule_providers: Vec::new(),
            node_order: Vec::new(),
            node_section_order: Vec::new(),
            group_order: Vec::new(),
        }
    }

    fn out(input: ConvertInput) -> Mapping {
        let yaml = convert(input).expect("conversion succeeds");
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[test]
    fn appends_nodes_replaces_groups_and_rules() {
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
            "DOMAIN-SUFFIX,example.com,MyGroup\nMATCH,DIRECT",
            nodes,
            groups,
        ));

        // Proxies: provider entries + appended custom nodes.
        let proxies = root.get("proxies").unwrap().as_sequence().unwrap();
        assert_eq!(names_in(root.get("proxies")), vec!["hk-1", "my-ss"]);
        assert_eq!(proxies.len(), 2);

        // proxy-groups: replaced with custom groups only — the provider's `Proxy`
        // group is dropped (not passed through).
        assert_eq!(names_in(root.get("proxy-groups")), vec!["MyGroup"]);

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
    fn custom_group_may_reuse_a_provider_group_name() {
        // Provider groups are replaced, so a custom group named `Proxy` (the
        // provider's group name) is allowed — this is exactly what importing a
        // provider group produces.
        let groups = vec![CustomGroup {
            name: "Proxy".into(),
            group_type: "select".into(),
            members: vec!["hk-1".into(), "DIRECT".into()],
            options: None,
        }];
        let root = out(input("MATCH,Proxy", vec![], groups));
        assert_eq!(names_in(root.get("proxy-groups")), vec!["Proxy"]);
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
            "DOMAIN,a.com,hk-1\nIP-CIDR,1.2.3.4/32,DIRECT,no-resolve\nMATCH,hk-1",
            vec![],
            vec![],
        ));
        assert_eq!(root.get("rules").unwrap().as_sequence().unwrap().len(), 3);
    }

    #[test]
    fn rule_targeting_unimported_provider_group_is_rejected() {
        // `Proxy` is a provider group; since provider groups are no longer in the
        // output, referencing one that wasn't imported is an unknown target.
        let err = convert(input("MATCH,Proxy", vec![], vec![])).unwrap_err();
        match err {
            ConvertError::Validation(errs) => {
                assert!(errs.iter().any(|e| e.contains("Proxy")));
            }
            _ => panic!("expected validation error"),
        }
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
            rule_providers: vec![],
            node_order: Vec::new(),
            node_section_order: Vec::new(),
            group_order: Vec::new(),
        };
        assert!(matches!(convert(bad), Err(ConvertError::ProviderParse)));
    }

    #[test]
    fn group_order_reorders_groups_and_appends_unlisted() {
        let groups = vec![
            CustomGroup {
                name: "G1".into(),
                group_type: "select".into(),
                members: vec!["hk-1".into()],
                options: None,
            },
            CustomGroup {
                name: "G2".into(),
                group_type: "select".into(),
                members: vec!["hk-1".into()],
                options: None,
            },
        ];
        // Default custom groups are [G1, G2] (provider groups are not output).
        // Ask for [G2]; `G1` is unlisted and must fall to the end.
        let mut inp = input("MATCH,DIRECT", vec![], groups);
        inp.group_order = vec!["G2".into()];
        let root = out(inp);
        assert_eq!(names_in(root.get("proxy-groups")), vec!["G2", "G1"]);
    }

    #[test]
    fn custom_rule_providers_merge_over_provider() {
        let mut inp = input("RULE-SET,my-block,REJECT\nMATCH,DIRECT", vec![], vec![]);
        inp.rule_providers = vec![
            // New custom 规则集.
            RuleProvider {
                name: "my-block".into(),
                provider_type: "http".into(),
                behavior: "domain".into(),
                options: Some(serde_json::json!({
                    "url": "https://example.com/block.yaml",
                    "format": "yaml",
                    "interval": 86400,
                })),
            },
            // Same name as a provider entry -> overrides it.
            RuleProvider {
                name: "ads".into(),
                provider_type: "inline".into(),
                behavior: "classical".into(),
                options: Some(serde_json::json!({ "payload": ["DOMAIN,ad.example.com"] })),
            },
        ];
        let root = out(inp);
        let rps = root.get("rule-providers").unwrap().as_mapping().unwrap();
        // Provider `ads` was overridden by the custom inline entry.
        let ads = rps.get("ads").unwrap().as_mapping().unwrap();
        assert_eq!(ads.get("type").unwrap().as_str().unwrap(), "inline");
        assert_eq!(ads.get("behavior").unwrap().as_str().unwrap(), "classical");
        // Custom `my-block` added alongside.
        let block = rps.get("my-block").unwrap().as_mapping().unwrap();
        assert_eq!(block.get("type").unwrap().as_str().unwrap(), "http");
        assert_eq!(
            block.get("url").unwrap().as_str().unwrap(),
            "https://example.com/block.yaml"
        );
    }

    fn node(name: &str) -> CustomNode {
        CustomNode {
            name: name.into(),
            content: format!("{{ name: {name}, type: ss, server: 9.9.9.9, port: 1080 }}"),
        }
    }

    #[test]
    fn node_order_reorders_only_the_custom_block() {
        // Provider block [hk-1] stays upstream; custom block [a, b] is reordered
        // by `node_order` (b first, a unlisted -> end). Default section order is
        // provider-first, so output = [hk-1] ++ [b, a].
        let mut inp = input("MATCH,DIRECT", vec![node("a"), node("b")], vec![]);
        inp.node_order = vec!["b".into()];
        let root = out(inp);
        assert_eq!(names_in(root.get("proxies")), vec!["hk-1", "b", "a"]);
    }

    #[test]
    fn node_section_order_puts_custom_block_first() {
        let mut inp = input("MATCH,DIRECT", vec![node("a"), node("b")], vec![]);
        inp.node_section_order = vec!["custom".into(), "provider".into()];
        let root = out(inp);
        assert_eq!(names_in(root.get("proxies")), vec!["a", "b", "hk-1"]);
    }

    #[test]
    fn empty_orders_keep_provider_first_then_custom() {
        let root = out(input("MATCH,DIRECT", vec![node("z")], vec![]));
        assert_eq!(names_in(root.get("proxies")), vec!["hk-1", "z"]);
    }
}
