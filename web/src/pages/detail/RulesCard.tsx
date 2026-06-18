import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  AutoComplete,
  Button,
  Card,
  Empty,
  Input,
  List,
  Popconfirm,
  Space,
  Switch,
  Tag,
  Typography,
  message,
} from "antd";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type {
  CustomGroup,
  CustomNode,
  ProviderRulesResponse,
  ProxiesResponse,
  RuleProvider,
} from "../../types";
import { BUILTIN_POLICIES } from "./groupSchema";

interface Props {
  profileId: string;
  initial: string;
  nodes: CustomNode[];
  groups: CustomGroup[];
  ruleProviders: RuleProvider[];
  /** Changes when the profile is (re)generated; refreshes policy suggestions. */
  generatedAt: string | null;
  /** Validation errors from the last generate attempt (itemized). */
  errors: string[];
  onSaved: () => void;
}

// Common Mihomo rule types (free text still allowed in the selector). Grouped
// by category for readability; the AutoComplete still accepts any typed value.
const RULE_TYPES = [
  // Domain
  "DOMAIN-SUFFIX",
  "DOMAIN",
  "DOMAIN-KEYWORD",
  "DOMAIN-REGEX",
  "GEOSITE",
  // IP
  "IP-CIDR",
  "IP-CIDR6",
  "IP-SUFFIX",
  "IP-ASN",
  "GEOIP",
  "SRC-GEOIP",
  "SRC-IP-ASN",
  "SRC-IP-CIDR",
  "SRC-IP-SUFFIX",
  // Port
  "DST-PORT",
  "SRC-PORT",
  "IN-PORT",
  // Process
  "PROCESS-NAME",
  "PROCESS-PATH",
  "PROCESS-NAME-REGEX",
  "PROCESS-PATH-REGEX",
  // Inbound / misc
  "IN-TYPE",
  "IN-USER",
  "IN-NAME",
  "UID",
  "NETWORK",
  "DSCP",
  "RULE-SET",
  // Logical / nested
  "AND",
  "OR",
  "NOT",
  "SUB-RULE",
  "MATCH",
];
// Types whose match resolves an IP and thus accept the `no-resolve` modifier.
const IP_TYPES = new Set([
  "IP-CIDR",
  "IP-CIDR6",
  "IP-SUFFIX",
  "IP-ASN",
  "GEOIP",
  "SRC-GEOIP",
  "SRC-IP-ASN",
  "SRC-IP-CIDR",
  "SRC-IP-SUFFIX",
  "RULE-SET",
]);
// Per-type payload example, shown as the content-input placeholder (à la Clash
// Verge). Helps the admin enter the right shape without docs.
const RULE_EXAMPLES: Record<string, string> = {
  DOMAIN: "example.com",
  "DOMAIN-SUFFIX": "example.com",
  "DOMAIN-KEYWORD": "google",
  "DOMAIN-REGEX": "^.*\\.example\\.com$",
  GEOSITE: "youtube",
  "IP-CIDR": "192.168.0.0/16",
  "IP-CIDR6": "2620:0:2d0:200::7/32",
  "IP-SUFFIX": "8.8.8.8/24",
  "IP-ASN": "13335",
  GEOIP: "CN",
  "SRC-GEOIP": "CN",
  "SRC-IP-ASN": "13335",
  "SRC-IP-CIDR": "192.168.1.0/24",
  "SRC-IP-SUFFIX": "192.168.1.0/24",
  "DST-PORT": "443",
  "SRC-PORT": "8080",
  "IN-PORT": "7890",
  "PROCESS-NAME": "curl",
  "PROCESS-PATH": "/usr/bin/curl",
  "PROCESS-NAME-REGEX": ".*curl.*",
  "PROCESS-PATH-REGEX": ".*/curl",
  "IN-TYPE": "SOCKS5",
  "IN-USER": "mihomo",
  "IN-NAME": "ss1",
  UID: "1000",
  DSCP: "0,32",
  AND: "(DOMAIN,example.com),(NETWORK,tcp)",
  OR: "(NETWORK,udp),(DST-PORT,443)",
  NOT: "(DOMAIN,example.com)",
};

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
  // Everything after the type is [payload..., policy, no-resolve?]. The payload
  // itself may contain commas (logical/nested rules like AND/OR/NOT), so peel the
  // optional trailing `no-resolve` and the policy off the end and re-join the
  // remainder as the payload rather than assuming fixed positions.
  let rest = parts.slice(1);
  let noResolve = false;
  if (rest.length > 0 && rest[rest.length - 1] === "no-resolve") {
    noResolve = true;
    rest = rest.slice(0, -1);
  }
  const policy = rest.length > 0 ? rest[rest.length - 1] : "";
  const payload = rest.slice(0, -1).join(",");
  return { type, payload, policy, noResolve };
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
  ruleProviders,
  generatedAt,
  errors,
  onSaved,
}: Props) {
  const { t } = useTranslation();
  // Source of truth is the raw, non-empty lines so comments and uncommon rules
  // (e.g. logical AND/OR) are preserved verbatim until explicitly edited.
  const [lines, setLines] = useState<string[]>([]);
  // The inline composer: editingIndex === null means "append a new rule",
  // otherwise we're rewriting the rule at that list position.
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [model, setModel] = useState<RuleModel>(EMPTY_RULE);
  const [policies, setPolicies] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

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

  // Rule order is semantic (first match wins), so reordering directly changes
  // behavior. Ids are list indices — stable within a render, which is all dnd
  // needs; on drop we reorder and persist the joined content.
  function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = Number(active.id);
    const newIndex = Number(over.id);
    const next = arrayMove(lines, oldIndex, newIndex);
    setLines(next); // optimistic; persist + reload reconciles
    void persist(next);
    message.success(t("rules.orderSaved"));
  }

  function resetComposer() {
    setEditingIndex(null);
    setModel(EMPTY_RULE);
  }

  function startEdit(index: number) {
    setEditingIndex(index);
    setModel(parseRule(lines[index]));
  }

  // Add (editingIndex === null) or save (rewrite at editingIndex) the composed
  // rule, then clear the composer back to "append" mode.
  function submit() {
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
    resetComposer();
    void persist(next);
  }

  function remove(index: number) {
    if (index === editingIndex) resetComposer();
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

  const upperType = model.type.trim().toUpperCase();
  const isMatch = upperType === "MATCH";
  const showNoResolve = !isMatch && IP_TYPES.has(upperType);

  // The content/payload input adapts to the selected rule type (à la Clash
  // Verge): RULE-SET picks a defined rule-set name, NETWORK picks tcp/udp,
  // everything else is a free text field with a per-type example placeholder.
  function contentInput() {
    if (upperType === "RULE-SET") {
      return (
        <AutoComplete
          style={{ width: 220 }}
          options={ruleProviders.map((rp) => ({ value: rp.name }))}
          value={model.payload}
          onChange={(payload) => setModel({ ...model, payload })}
          placeholder={t("rules.ruleSetPayloadHint")}
          filterOption={(input, opt) =>
            String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      );
    }
    if (upperType === "NETWORK") {
      return (
        <AutoComplete
          style={{ width: 220 }}
          options={[{ value: "tcp" }, { value: "udp" }]}
          value={model.payload}
          onChange={(payload) => setModel({ ...model, payload })}
          placeholder="tcp / udp"
        />
      );
    }
    return (
      <Input
        style={{ width: 220 }}
        value={model.payload}
        onChange={(e) => setModel({ ...model, payload: e.target.value })}
        placeholder={RULE_EXAMPLES[upperType] ?? t("rules.payload")}
      />
    );
  }

  return (
    <Card
      title={`${t("rules.title")} (${lines.length})`}
      extra={
        <Popconfirm title={t("rules.importConfirm")} onConfirm={importProviderRules}>
          <Button loading={importing}>{t("rules.importProvider")}</Button>
        </Popconfirm>
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

        {/* Inline composer (Clash Verge style): type · content · no-resolve · policy. */}
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: 8,
            alignItems: "center",
            padding: "8px 0",
          }}
        >
          <AutoComplete
            style={{ width: 200 }}
            options={RULE_TYPES.map((v) => ({ value: v }))}
            value={model.type}
            onChange={(type) => setModel({ ...model, type })}
            placeholder={t("rules.ruleType")}
            filterOption={(input, opt) =>
              (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
            }
          />
          {!isMatch && contentInput()}
          {showNoResolve && (
            <Space size={4}>
              <Switch
                size="small"
                checked={model.noResolve}
                onChange={(noResolve) => setModel({ ...model, noResolve })}
              />
              <Typography.Text type="secondary">no-resolve</Typography.Text>
            </Space>
          )}
          <AutoComplete
            style={{ width: 200 }}
            options={policyOptions}
            value={model.policy}
            onChange={(policy) => setModel({ ...model, policy })}
            placeholder={t("rules.policy")}
            filterOption={(input, opt) =>
              String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
            }
          />
          <Button type="primary" onClick={submit}>
            {editingIndex === null ? t("rules.add") : t("common.save")}
          </Button>
          {editingIndex !== null && <Button onClick={resetComposer}>{t("common.cancel")}</Button>}
        </div>

        {lines.length === 0 ? (
          <Empty description={t("rules.empty")} />
        ) : (
          <>
            <Typography.Text type="secondary">{t("rules.dragHint")}</Typography.Text>
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
              <SortableContext
                items={lines.map((_, i) => String(i))}
                strategy={verticalListSortingStrategy}
              >
                <List>
                  {lines.map((line, index) => (
                    <SortableRuleItem
                      key={index}
                      id={String(index)}
                      line={line}
                      active={index === editingIndex}
                      onEdit={() => startEdit(index)}
                      onRemove={() => remove(index)}
                    />
                  ))}
                </List>
              </SortableContext>
            </DndContext>
          </>
        )}
      </Space>
    </Card>
  );
}

interface RuleItemProps {
  id: string;
  line: string;
  active: boolean;
  onEdit: () => void;
  onRemove: () => void;
}

function SortableRuleItem({ id, line, active, onEdit, onRemove }: RuleItemProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "rgba(0,0,0,0.04)" : active ? "rgba(22,119,255,0.08)" : undefined,
  };
  const comment = isComment(line);
  const r = parseRule(line);

  const actions = comment
    ? [
        <Popconfirm key="del" title={t("rules.deleteConfirm")} onConfirm={onRemove}>
          <a>{t("rules.delete")}</a>
        </Popconfirm>,
      ]
    : [
        <a key="edit" onClick={onEdit}>
          {t("basic.edit")}
        </a>,
        <Popconfirm key="del" title={t("rules.deleteConfirm")} onConfirm={onRemove}>
          <a>{t("rules.delete")}</a>
        </Popconfirm>,
      ];

  return (
    <List.Item ref={setNodeRef} style={style} actions={actions}>
      <Space>
        <span
          {...attributes}
          {...listeners}
          style={{ cursor: "grab", color: "#999", userSelect: "none", touchAction: "none" }}
          aria-label="drag handle"
        >
          ⋮⋮
        </span>
        {comment ? (
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
      </Space>
    </List.Item>
  );
}
