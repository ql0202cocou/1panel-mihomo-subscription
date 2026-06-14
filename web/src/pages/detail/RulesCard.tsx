import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  AutoComplete,
  Button,
  Card,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Popconfirm,
  Space,
  Switch,
  Tag,
  Typography,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type {
  CustomGroup,
  CustomNode,
  ProviderRulesResponse,
  ProxiesResponse,
} from "../../types";
import { BUILTIN_POLICIES } from "./groupSchema";

interface Props {
  profileId: string;
  initial: string;
  nodes: CustomNode[];
  groups: CustomGroup[];
  /** Changes when the profile is (re)generated; refreshes policy suggestions. */
  generatedAt: string | null;
  /** Validation errors from the last generate attempt (itemized). */
  errors: string[];
  onSaved: () => void;
}

// Common Mihomo rule types (free text still allowed in the selector).
const RULE_TYPES = [
  "DOMAIN-SUFFIX",
  "DOMAIN",
  "DOMAIN-KEYWORD",
  "DOMAIN-REGEX",
  "GEOSITE",
  "IP-CIDR",
  "IP-CIDR6",
  "GEOIP",
  "IP-ASN",
  "SRC-IP-CIDR",
  "DST-PORT",
  "SRC-PORT",
  "PROCESS-NAME",
  "PROCESS-PATH",
  "RULE-SET",
  "MATCH",
];
// Types whose match resolves an IP and thus accept the `no-resolve` modifier.
const IP_TYPES = new Set(["IP-CIDR", "IP-CIDR6", "GEOIP", "IP-ASN", "SRC-IP-CIDR"]);

interface RuleModel {
  type: string;
  payload: string;
  policy: string;
  noResolve: boolean;
}

const EMPTY_RULE: RuleModel = { type: "DOMAIN-SUFFIX", payload: "", policy: "", noResolve: false };

function parseRule(line: string): RuleModel {
  const parts = line.split(",").map((p) => p.trim());
  const type = parts[0] ?? "";
  if (type.toUpperCase() === "MATCH") {
    return { type, payload: "", policy: parts[1] ?? "", noResolve: false };
  }
  return {
    type,
    payload: parts[1] ?? "",
    policy: parts[2] ?? "",
    noResolve: parts.slice(3).some((p) => p === "no-resolve"),
  };
}

function serializeRule(r: RuleModel): string {
  if (r.type.toUpperCase() === "MATCH") return `MATCH,${r.policy}`;
  const base = `${r.type},${r.payload},${r.policy}`;
  return r.noResolve ? `${base},no-resolve` : base;
}

const isComment = (line: string) => line.startsWith("#");

export default function RulesCard({
  profileId,
  initial,
  nodes,
  groups,
  generatedAt,
  errors,
  onSaved,
}: Props) {
  const { t } = useTranslation();
  // Source of truth is the raw, non-empty lines so comments and uncommon rules
  // (e.g. logical AND/OR) are preserved verbatim until explicitly edited.
  const [lines, setLines] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [model, setModel] = useState<RuleModel>(EMPTY_RULE);
  const [policies, setPolicies] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    setLines(
      initial
        .split("\n")
        .map((l) => l.trim())
        .filter((l) => l !== ""),
    );
  }, [initial]);

  const loadPolicies = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setPolicies([...res.proxies.map((p) => p.name), ...res.groups.map((g) => g.name)]);
    } catch {
      // Non-fatal: policies can still be typed in by hand.
    }
  }, [profileId]);

  useEffect(() => {
    void loadPolicies();
  }, [loadPolicies, generatedAt]);

  async function persist(next: string[]) {
    try {
      await api(`/api/profiles/${profileId}/rules`, {
        method: "PUT",
        body: JSON.stringify({ content: next.join("\n") }),
      });
      onSaved();
    } catch (e) {
      message.error((e as ApiError).message ?? t("common.saveFailed"));
    }
  }

  function startAdd() {
    setEditingIndex(null);
    setModel(EMPTY_RULE);
    setOpen(true);
  }

  function startEdit(index: number) {
    setEditingIndex(index);
    setModel(parseRule(lines[index]));
    setOpen(true);
  }

  function save() {
    const isMatch = model.type.trim().toUpperCase() === "MATCH";
    if (!model.type.trim() || !model.policy.trim() || (!isMatch && !model.payload.trim())) {
      message.error(t("rules.incomplete"));
      return;
    }
    const line = serializeRule(model);
    const next =
      editingIndex === null
        ? [...lines, line]
        : lines.map((l, i) => (i === editingIndex ? line : l));
    setOpen(false);
    void persist(next);
  }

  function remove(index: number) {
    void persist(lines.filter((_, i) => i !== index));
  }

  // Seed the editor with the airport's own rules (the converter otherwise
  // replaces provider rules). Appends, skipping lines already present.
  async function importProviderRules() {
    setImporting(true);
    try {
      const res = await api<ProviderRulesResponse>(`/api/profiles/${profileId}/provider-rules`);
      const existing = new Set(lines);
      const incoming = res.rules
        .map((l) => l.trim())
        .filter((l) => l !== "" && !existing.has(l));
      if (incoming.length === 0) {
        message.info(t("rules.importNone"));
        return;
      }
      await persist([...lines, ...incoming]);
      message.success(t("rules.imported", { count: incoming.length }));
    } catch (e) {
      message.error((e as ApiError).message ?? t("rules.importFailed"));
    } finally {
      setImporting(false);
    }
  }

  const policyOptions = Array.from(
    new Set(
      [
        ...policies,
        ...nodes.map((n) => n.name),
        ...groups.map((g) => g.name),
        ...BUILTIN_POLICIES,
      ].filter((s) => s.trim() !== ""),
    ),
  ).map((value) => ({ value }));

  const isMatch = model.type.trim().toUpperCase() === "MATCH";

  return (
    <Card
      title={`${t("rules.title")} (${lines.length})`}
      extra={
        <Space>
          <Popconfirm title={t("rules.importConfirm")} onConfirm={importProviderRules}>
            <Button loading={importing}>{t("rules.importProvider")}</Button>
          </Popconfirm>
          <Button onClick={startAdd}>{t("rules.add")}</Button>
        </Space>
      }
    >
      <Space direction="vertical" style={{ width: "100%" }} size="small">
        <Typography.Text type="secondary">{t("rules.hint")}</Typography.Text>
        {errors.length > 0 && (
          <Alert
            type="error"
            showIcon
            message={t("rules.invalid")}
            description={errors.map((err, i) => (
              <div key={i}>{err}</div>
            ))}
          />
        )}
        {lines.length === 0 ? (
          <Empty description={t("rules.empty")} />
        ) : (
          <List>
            {lines.map((line, index) => {
              const r = parseRule(line);
              return (
                <List.Item
                  key={`${index}-${line}`}
                  actions={
                    isComment(line)
                      ? [
                          <Popconfirm
                            key="del"
                            title={t("rules.deleteConfirm")}
                            onConfirm={() => remove(index)}
                          >
                            <a>{t("rules.delete")}</a>
                          </Popconfirm>,
                        ]
                      : [
                          <a key="edit" onClick={() => startEdit(index)}>
                            {t("basic.edit")}
                          </a>,
                          <Popconfirm
                            key="del"
                            title={t("rules.deleteConfirm")}
                            onConfirm={() => remove(index)}
                          >
                            <a>{t("rules.delete")}</a>
                          </Popconfirm>,
                        ]
                  }
                >
                  {isComment(line) ? (
                    <Typography.Text type="secondary">{line}</Typography.Text>
                  ) : (
                    <Space wrap>
                      <Tag>{r.type}</Tag>
                      {r.type.toUpperCase() !== "MATCH" && <span>{r.payload}</span>}
                      <span style={{ color: "#999" }}>→</span>
                      <Tag color="blue">{r.policy}</Tag>
                      {r.noResolve && <Tag>no-resolve</Tag>}
                    </Space>
                  )}
                </List.Item>
              );
            })}
          </List>
        )}
      </Space>

      <Modal
        title={editingIndex === null ? t("rules.add") : t("rules.edit")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={560}
        destroyOnClose
      >
        <Form layout="vertical">
          <Form.Item label={t("rules.ruleType")} required>
            <AutoComplete
              style={{ width: "100%" }}
              options={RULE_TYPES.map((v) => ({ value: v }))}
              value={model.type}
              onChange={(type) => setModel({ ...model, type })}
              filterOption={(input, opt) =>
                (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>
          {!isMatch && (
            <Form.Item label={t("rules.payload")} required>
              <Input
                value={model.payload}
                onChange={(e) => setModel({ ...model, payload: e.target.value })}
                placeholder="example.com / 1.2.3.4/24 / CN"
              />
            </Form.Item>
          )}
          <Form.Item label={t("rules.policy")} required>
            <AutoComplete
              style={{ width: "100%" }}
              options={policyOptions}
              value={model.policy}
              onChange={(policy) => setModel({ ...model, policy })}
              filterOption={(input, opt) =>
                String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>
          {!isMatch && IP_TYPES.has(model.type.trim().toUpperCase()) && (
            <Form.Item label={t("rules.noResolve")}>
              <Switch
                checked={model.noResolve}
                onChange={(noResolve) => setModel({ ...model, noResolve })}
              />
            </Form.Item>
          )}
        </Form>
      </Modal>
    </Card>
  );
}
