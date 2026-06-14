// Structured-editor schema for custom proxy nodes. Each node's stored `content`
// is a full Mihomo proxy YAML mapping; the editor exposes the common fields per
// type as typed inputs (including nested option blocks like REALITY / ws-opts /
// grpc-opts) and lets every other key be edited as an advanced key/value row, so
// an admin never has to hand-write YAML for the usual cases.

export type FieldKind = "text" | "password" | "number" | "switch" | "select" | "tags";

export interface FieldDef {
  key: string;
  kind: FieldKind;
  /** Suggestions for `select`/`tags` fields (free text still allowed). */
  options?: string[];
  placeholder?: string;
  /** Show this field only when the predicate holds against the current fields. */
  showWhen?: (fields: Record<string, unknown>) => boolean;
}

/** A nested option object (e.g. `reality-opts`) edited as a titled sub-section. */
export interface GroupDef {
  /** The proxy key holding the nested object. */
  key: string;
  fields: FieldDef[];
  showWhen?: (fields: Record<string, unknown>) => boolean;
}

interface TypeSchema {
  fields: FieldDef[];
  groups?: GroupDef[];
}

// ─── Predicates for conditional display ──────────────────────────────────────
const tlsOn = (f: Record<string, unknown>) => f.tls === true;
const networkIs =
  (...nets: string[]) =>
  (f: Record<string, unknown>) =>
    nets.includes(String(f.network ?? "tcp"));

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
const FINGERPRINTS = ["chrome", "firefox", "safari", "ios", "android", "edge", "random"];
const ALPN = ["h2", "http/1.1", "h3"];

// Transport option blocks shared by vmess/vless/trojan.
const WS_OPTS: GroupDef = {
  key: "ws-opts",
  showWhen: networkIs("ws"),
  fields: [
    { key: "path", kind: "text", placeholder: "/" },
    { key: "headers.Host", kind: "text" },
  ],
};
const GRPC_OPTS: GroupDef = {
  key: "grpc-opts",
  showWhen: networkIs("grpc"),
  fields: [{ key: "grpc-service-name", kind: "text" }],
};
// REALITY: TLS with a server-provided public key + short id.
const REALITY_OPTS: GroupDef = {
  key: "reality-opts",
  showWhen: tlsOn,
  fields: [
    { key: "public-key", kind: "text" },
    { key: "short-id", kind: "text" },
  ],
};

/** Type-specific common fields + nested groups. Anything else falls to KV. */
const TYPE_SCHEMA: Record<string, TypeSchema> = {
  ss: {
    fields: [
      { key: "cipher", kind: "select", options: SS_CIPHERS },
      { key: "password", kind: "password" },
      { key: "udp", kind: "switch" },
    ],
  },
  vmess: {
    fields: [
      { key: "uuid", kind: "text" },
      { key: "alterId", kind: "number" },
      { key: "cipher", kind: "select", options: VMESS_CIPHERS },
      { key: "udp", kind: "switch" },
      { key: "network", kind: "select", options: NETWORKS },
      { key: "tls", kind: "switch" },
      { key: "servername", kind: "text", showWhen: tlsOn },
      { key: "alpn", kind: "tags", options: ALPN, showWhen: tlsOn },
      { key: "skip-cert-verify", kind: "switch", showWhen: tlsOn },
      { key: "client-fingerprint", kind: "select", options: FINGERPRINTS, showWhen: tlsOn },
    ],
    groups: [WS_OPTS, GRPC_OPTS],
  },
  vless: {
    fields: [
      { key: "uuid", kind: "text" },
      { key: "flow", kind: "select", options: ["xtls-rprx-vision"] },
      { key: "udp", kind: "switch" },
      { key: "network", kind: "select", options: NETWORKS },
      { key: "tls", kind: "switch" },
      { key: "servername", kind: "text", showWhen: tlsOn },
      { key: "alpn", kind: "tags", options: ALPN, showWhen: tlsOn },
      { key: "skip-cert-verify", kind: "switch", showWhen: tlsOn },
      { key: "client-fingerprint", kind: "select", options: FINGERPRINTS, showWhen: tlsOn },
    ],
    // REALITY shown when TLS is on; ws/grpc shown by network.
    groups: [REALITY_OPTS, WS_OPTS, GRPC_OPTS],
  },
  trojan: {
    fields: [
      { key: "password", kind: "password" },
      { key: "sni", kind: "text" },
      { key: "alpn", kind: "tags", options: ALPN },
      { key: "skip-cert-verify", kind: "switch" },
      { key: "client-fingerprint", kind: "select", options: FINGERPRINTS },
      { key: "network", kind: "select", options: NETWORKS },
    ],
    groups: [WS_OPTS, GRPC_OPTS],
  },
  hysteria2: {
    fields: [
      { key: "password", kind: "password" },
      { key: "sni", kind: "text" },
      { key: "skip-cert-verify", kind: "switch" },
      { key: "up", kind: "text", placeholder: "30 Mbps" },
      { key: "down", kind: "text", placeholder: "200 Mbps" },
      { key: "obfs", kind: "select", options: ["salamander"] },
      { key: "obfs-password", kind: "password" },
      { key: "alpn", kind: "tags", options: ALPN },
    ],
  },
  tuic: {
    fields: [
      { key: "uuid", kind: "text" },
      { key: "password", kind: "password" },
      { key: "sni", kind: "text" },
      { key: "alpn", kind: "tags", options: ALPN },
      { key: "skip-cert-verify", kind: "switch" },
      { key: "congestion-controller", kind: "select", options: ["bbr", "cubic", "new_reno"] },
    ],
  },
  http: {
    fields: [
      { key: "username", kind: "text" },
      { key: "password", kind: "password" },
      { key: "tls", kind: "switch" },
      { key: "skip-cert-verify", kind: "switch", showWhen: tlsOn },
    ],
  },
  socks5: {
    fields: [
      { key: "username", kind: "text" },
      { key: "password", kind: "password" },
      { key: "tls", kind: "switch" },
      { key: "skip-cert-verify", kind: "switch", showWhen: tlsOn },
    ],
  },
};

/** Common fields (base + type-specific) for a given proxy type. */
export function commonFields(type: string): FieldDef[] {
  return [...BASE_FIELDS, ...(TYPE_SCHEMA[type]?.fields ?? [])];
}

/** Nested option groups (reality-opts, ws-opts, …) for a given proxy type. */
export function groupsFor(type: string): GroupDef[] {
  return TYPE_SCHEMA[type]?.groups ?? [];
}

/** Keys owned by dedicated inputs/groups (so advanced KV can exclude them). */
export function commonKeys(type: string): Set<string> {
  return new Set([
    "name",
    "type",
    ...commonFields(type).map((f) => f.key),
    ...groupsFor(type).map((g) => g.key),
  ]);
}
