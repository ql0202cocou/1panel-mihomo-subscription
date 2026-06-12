//! Bounded YAML parsing for untrusted/admin-submitted documents.
//!
//! Per `docs/security-design.md`, YAML from outside the trust boundary (admin
//! node/group content here; provider content in the converter) must be parsed
//! with resource limits. The request body is already capped (1 MB), and here we
//! additionally reject documents whose nesting depth or node count is
//! implausibly large, bounding the work done on the parsed structure.
//!
//! NOTE: alias-expansion ("billion laughs") amplification happens inside
//! `serde_yaml` during parse. The 1 MB input cap bounds the blast radius; the
//! converter task revisits a parser with explicit alias caps for provider
//! content fetched at scale.

use serde_yaml::Value;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 10_000;

#[derive(Debug, PartialEq, Eq)]
pub enum YamlError {
    Parse,
    TooComplex,
    NotMapping,
}

/// Parse `text` into a YAML value, enforcing depth and node-count limits.
pub fn parse_limited(text: &str) -> Result<Value, YamlError> {
    let value: Value = serde_yaml::from_str(text).map_err(|_| YamlError::Parse)?;
    let mut nodes = 0usize;
    check(&value, 1, &mut nodes)?;
    Ok(value)
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
}
