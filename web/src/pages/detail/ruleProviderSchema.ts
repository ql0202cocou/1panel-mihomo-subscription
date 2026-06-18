// Structured-editor schema for custom rule-providers (规则集). `type` and
// `behavior` are first-class selects; the remaining keys (url, path, payload,
// format, interval, …) are typed inputs per provider type, with anything else
// falling to advanced key/value rows — no hand-written JSON.

import type { FieldDef } from "./nodeSchema";
import type { RuleProviderType, RuleProviderBehavior } from "../../types";

export const RP_TYPES: RuleProviderType[] = ["http", "file", "inline"];
export const RP_BEHAVIORS: RuleProviderBehavior[] = ["domain", "ipcidr", "classical"];

const FORMAT: FieldDef = {
  key: "format",
  kind: "select",
  options: ["yaml", "text", "mrs"],
  placeholder: "yaml",
};

/** Type-specific option fields; anything else falls to advanced KV. */
export const RP_OPTION_FIELDS: Record<string, FieldDef[]> = {
  http: [
    { key: "url", kind: "text", placeholder: "https://example.com/rules.yaml" },
    FORMAT,
    { key: "interval", kind: "number", placeholder: "86400" },
    { key: "proxy", kind: "text" },
    { key: "size-limit", kind: "number" },
  ],
  file: [
    { key: "path", kind: "text", placeholder: "./rules/custom.yaml" },
    FORMAT,
  ],
  inline: [
    { key: "payload", kind: "tags", placeholder: "DOMAIN-SUFFIX,example.com" },
  ],
};

export function rpOptionFields(type: string): FieldDef[] {
  return RP_OPTION_FIELDS[type] ?? [];
}

export function rpOptionKeys(type: string): Set<string> {
  return new Set(rpOptionFields(type).map((f) => f.key));
}
