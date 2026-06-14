// Structured-editor schema for custom proxy-group options. The group's options
// (url, interval, strategy, …) are edited as typed inputs per group type, with
// everything else falling to advanced key/value rows — no hand-written JSON.

import type { FieldDef } from "./nodeSchema";
import type { GroupType } from "../../types";

export const GROUP_TYPES: GroupType[] = [
  "select",
  "url-test",
  "fallback",
  "load-balance",
  "relay",
];

const HEALTH: FieldDef[] = [
  { key: "url", kind: "text", placeholder: "https://www.gstatic.com/generate_204" },
  { key: "interval", kind: "number", placeholder: "300" },
];

/** Type-specific common option fields; anything else falls to advanced KV. */
export const GROUP_OPTION_FIELDS: Record<string, FieldDef[]> = {
  select: [],
  "url-test": [
    ...HEALTH,
    { key: "tolerance", kind: "number" },
    { key: "lazy", kind: "switch" },
  ],
  fallback: [...HEALTH, { key: "lazy", kind: "switch" }],
  "load-balance": [
    ...HEALTH,
    {
      key: "strategy",
      kind: "select",
      options: ["consistent-hashing", "round-robin", "sticky-sessions"],
    },
  ],
  relay: [],
};

export function groupOptionFields(type: string): FieldDef[] {
  return GROUP_OPTION_FIELDS[type] ?? [];
}

export function groupOptionKeys(type: string): Set<string> {
  return new Set(groupOptionFields(type).map((f) => f.key));
}

/** Built-in policy targets always valid as group members. */
export const BUILTIN_POLICIES = [
  "DIRECT",
  "REJECT",
  "REJECT-DROP",
  "PASS",
  "COMPATIBLE",
  "GLOBAL",
];
