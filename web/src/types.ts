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
  created_at: string;
  updated_at: string;
}

export interface Settings {
  public_path_prefix: string;
}
