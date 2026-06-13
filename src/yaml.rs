//! Bounded YAML parsing for untrusted/admin-submitted documents.
//!
//! Per `docs/security-design.md`, YAML from outside the trust boundary (admin
//! node/group content here; provider content in the converter) must be parsed
//! with resource limits: nesting depth and node count after parse, plus an
//! anchor/alias cap *before* parse.
//!
//! The anchor/alias cap is the defense against alias-expansion ("billion
//! laughs"): such inputs are tiny, so the size cap and post-parse checks cannot
//! help — `serde_yaml` would already have expanded the bomb (and OOM'd) inside
//! `from_str`. So we scan the raw text first and reject documents with an
//! implausible number of anchors/aliases, bounding worst-case expansion to a
//! size the post-parse node-count check can then reject safely.

use serde_yaml::Value;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 10_000;
/// Max combined `&anchor` definitions and `*alias` references. A doubling
/// alias chain uses ~3 tokens per level, so 32 bounds expansion to ~2^10
/// nodes. Legitimate Mihomo configs use few or no anchors.
const MAX_ANCHORS_ALIASES: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum YamlError {
    Parse,
    TooComplex,
    NotMapping,
}

/// Parse `text` into a YAML value, enforcing the anchor/alias cap before parse
/// and depth/node-count limits after.
pub fn parse_limited(text: &str) -> Result<Value, YamlError> {
    if count_anchors_aliases(text) > MAX_ANCHORS_ALIASES {
        return Err(YamlError::TooComplex);
    }
    let value: Value = serde_yaml::from_str(text).map_err(|_| YamlError::Parse)?;
    let mut nodes = 0usize;
    check(&value, 1, &mut nodes)?;
    Ok(value)
}

/// Count YAML anchor (`&name`) and alias (`*name`) tokens in the raw text. To
/// avoid counting `&`/`*` inside scalars, only sigils preceded by whitespace or
/// a flow indicator and followed by a name character are counted.
fn count_anchors_aliases(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    for i in 0..bytes.len() {
        let c = bytes[i];
        if c != b'&' && c != b'*' {
            continue;
        }
        let prev_ok = i == 0
            || matches!(
                bytes[i - 1],
                b' ' | b'\t' | b'\n' | b'\r' | b'[' | b'{' | b','
            );
        let next_ok = bytes
            .get(i + 1)
            .is_some_and(|n| n.is_ascii_alphanumeric() || *n == b'_' || *n == b'-');
        if prev_ok && next_ok {
            count += 1;
        }
    }
    count
}

/// Parse `text` and require the top level to be a mapping (e.g. a single Mihomo
/// proxy definition).
pub fn parse_mapping(text: &str) -> Result<serde_yaml::Mapping, YamlError> {
    match parse_limited(text)? {
        Value::Mapping(map) => Ok(map),
        _ => Err(YamlError::NotMapping),
    }
}

fn check(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), YamlError> {
    if depth > MAX_DEPTH {
        return Err(YamlError::TooComplex);
    }
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(YamlError::TooComplex);
    }
    match value {
        Value::Sequence(seq) => {
            for item in seq {
                check(item, depth + 1, nodes)?;
            }
        }
        Value::Mapping(map) => {
            for (_, v) in map {
                check(v, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_simple_proxy_mapping() {
        let yaml = "name: my-ss\ntype: ss\nserver: 1.2.3.4\nport: 8388";
        assert!(parse_mapping(yaml).is_ok());
    }

    #[test]
    fn rejects_non_mapping_top_level() {
        assert_eq!(parse_mapping("- a\n- b"), Err(YamlError::NotMapping));
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert_eq!(parse_limited(":\n  - ["), Err(YamlError::Parse));
    }

    #[test]
    fn allows_light_anchor_use() {
        let yaml = "defaults: &d { type: ss, port: 8388 }\nnode: { name: a, <<: *d }";
        assert!(parse_limited(yaml).is_ok());
    }

    #[test]
    fn rejects_billion_laughs_before_parsing() {
        // A small input that would expand exponentially; rejected by the
        // anchor/alias cap before serde_yaml can materialize it.
        let mut yaml = String::from("a: &a [x, x, x, x, x, x, x, x, x, x]\n");
        for (level, prev) in [('b', 'a'), ('c', 'b'), ('d', 'c'), ('e', 'd')] {
            yaml.push_str(&format!(
                "{level}: &{level} [*{prev}, *{prev}, *{prev}, *{prev}, *{prev}, *{prev}, *{prev}, *{prev}, *{prev}, *{prev}]\n"
            ));
        }
        assert_eq!(parse_limited(&yaml), Err(YamlError::TooComplex));
    }
}
