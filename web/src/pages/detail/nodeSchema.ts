// Structured-editor schema for custom proxy nodes. Each node's stored `content`
// is a full Mihomo proxy YAML mapping; the editor exposes the common fields per
// type as typed inputs and lets every other key be edited as an advanced
// key/value row, so an admin never has to hand-write YAML for the usual cases.

export type FieldKind = "text" | "password" | "number" | "switch" | "select";

export interface FieldDef {
  key: string;
  kind: FieldKind;
  /** Suggestions for `select` fields (free text still allowed). */
  options?: string[];
  placeholder?: string;
}

/** Proxy types offered as suggestions in the type selector (free text allowed). */
export const NODE_TYPES = [
  "ss",
  "ssr",
  "vmess",
  "vless",
  "trojan",
  "hysteria2",
  "hysteria",
  "tuic",
  "wireguard",
  "http",
  "socks5",
  "snell",
];

/** Fields shown for every type, before the type-specific ones. */
export const BASE_FIELDS: FieldDef[] = [
  { key: "server", kind: "text", placeholder: "example.com / 1.2.3.4" },
  { key: "port", kind: "number" },
];

const SS_CIPHERS = [
  "aes-128-gcm",
  "aes-256-gcm",
  "chacha20-ietf-poly1305",
  "2022-blake3-aes-256-gcm",
  "none",
];
const VMESS_CIPHERS = ["auto", "none", "zero", "aes-128-gcm", "chacha20-poly1305"];
const NETWORKS = ["tcp", "ws", "grpc", "h2", "http"];

/** Type-specific common fields. Anything not listed here falls to advanced KV. */
export const TYPE_FIELDS: Record<string, FieldDef[]> = {
  ss: [
    { key: "cipher", kind: "select", options: SS_CIPHERS },
    { key: "password", kind: "password" },
  ],
  vmess: [
    { key: "uuid", kind: "text" },
    { key: "alterId", kind: "number" },
    { key: "cipher", kind: "select", options: VMESS_CIPHERS },
    { key: "network", kind: "select", options: NETWORKS },
    { key: "tls", kind: "switch" },
    { key: "servername", kind: "text" },
  ],
  vless: [
    { key: "uuid", kind: "text" },
    { key: "flow", kind: "text", placeholder: "xtls-rprx-vision" },
    { key: "network", kind: "select", options: NETWORKS },
    { key: "tls", kind: "switch" },
    { key: "servername", kind: "text" },
  ],
  trojan: [
    { key: "password", kind: "password" },
    { key: "sni", kind: "text" },
    { key: "skip-cert-verify", kind: "switch" },
  ],
  hysteria2: [
    { key: "password", kind: "password" },
    { key: "sni", kind: "text" },
    { key: "skip-cert-verify", kind: "switch" },
    { key: "up", kind: "text", placeholder: "30 Mbps" },
    { key: "down", kind: "text", placeholder: "200 Mbps" },
    { key: "obfs", kind: "text" },
  ],
  tuic: [
    { key: "uuid", kind: "text" },
    { key: "password", kind: "password" },
    { key: "sni", kind: "text" },
  ],
  http: [
    { key: "username", kind: "text" },
    { key: "password", kind: "password" },
    { key: "tls", kind: "switch" },
  ],
  socks5: [
    { key: "username", kind: "text" },
    { key: "password", kind: "password" },
    { key: "tls", kind: "switch" },
  ],
};

/** Common fields (base + type-specific) for a given proxy type. */
export function commonFields(type: string): FieldDef[] {
  return [...BASE_FIELDS, ...(TYPE_FIELDS[type] ?? [])];
}

/** Keys owned by dedicated inputs for a type (so advanced KV can exclude them). */
export function commonKeys(type: string): Set<string> {
  return new Set(["name", "type", ...commonFields(type).map((f) => f.key)]);
}
