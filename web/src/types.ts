export type SourceType = "mihomo" | "clash" | "surge" | "loon";

export interface ProfileSummary {
  id: string;
  name: string;
  source_type: SourceType;
  source_url_masked: string;
  output_type: string;
  enabled: boolean;
  subscription_url: string;
  last_fetch_at: string | null;
  last_fetch_status: string | null;
  last_generated_at: string | null;
  created_at: string;
  updated_at: string;
}

export type GroupType =
  | "select"
  | "url-test"
  | "fallback"
  | "load-balance"
  | "relay";

export interface RuleSet {
  content: string;
  updated_at: string;
}

export interface CustomNode {
  id: string;
  name: string;
  node_type: string;
  content: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

/** A proxy as it appears in the generated output (provider or custom). */
export interface ProxyPreview {
  name: string;
  type: string;
}

export interface ProxiesResponse {
  generated: boolean;
  generated_at: string | null;
  proxies: ProxyPreview[];
  /** Provider proxy-groups (name + type) from the latest generated output. */
  groups: ProxyPreview[];
}

export interface CustomGroup {
  id: string;
  name: string;
  group_type: GroupType;
  members: string[];
  options: Record<string, unknown> | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProfileDetail extends ProfileSummary {
  rules: RuleSet | null;
  nodes: CustomNode[];
  groups: CustomGroup[];
}

export interface Settings {
  public_path_prefix: string;
}
