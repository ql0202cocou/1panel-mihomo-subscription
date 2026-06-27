export interface ProfileSummary {
  id: string;
  name: string;
  source_url_masked: string;
  output_type: string;
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

export interface ProviderRulesResponse {
  rules: string[];
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

/** 生成结果中出现的一个代理(来自机场或自定义)。 */
export interface ProxyPreview {
  name: string;
  type: string;
}

export interface ProxiesResponse {
  generated: boolean;
  generated_at: string | null;
  proxies: ProxyPreview[];
  /** 两个节点区块的顺序,如 ["provider","custom"]。 */
  node_section_order: string[];
  /** 最近一次生成结果中的机场代理组(名称 + 类型)。 */
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

/** 全局规则集(「规则托管」):面板托管,订阅以 RULE-SET 引用。 */
export interface RuleSet {
  id: string;
  name: string;
  behavior: "domain" | "ipcidr" | "classical";
  format: "yaml" | "text" | "mrs";
  source: "manual" | "remote";
  content: string;
  enabled: boolean;
  /** 规则条数(manual=payload 行数;remote=最近成功镜像的行数)。 */
  count: number;
  /** 面板托管链接。 */
  url: string;
  /** 远程来源 URL(已脱敏);manual 为 null。 */
  remote_url_masked: string | null;
  interval_hours: number;
  /** 远程是否本地缓存二次托管。 */
  cache: boolean;
  last_fetch_status: string | null;
  created_at: string;
  updated_at: string;
}

export interface Settings {
  public_path_prefix: string;
}
