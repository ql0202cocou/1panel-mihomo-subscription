import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Alert,
  AutoComplete,
  Button,
  Card,
  Checkbox,
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

// Common Mihomo rule types (free text still allowed in the selector). `MATCH` is
// intentionally absent — the fallback policy is edited via its own dedicated
// control so it can never be dragged out of last position.
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

const isComment = (line: string) => line.startsWith("#");
const isMatchLine = (line: string) =>
  !isComment(line) && parseRule(line).type.trim().toUpperCase() === "MATCH";

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
  // Source of truth is the raw, non-empty lines *excluding* the MATCH fallback
  // (which is edited separately so it always stays last). Comments and uncommon
  // rules (e.g. logical AND/OR) are preserved verbatim until explicitly edited.
  const [lines, setLines] = useState<string[]>([]);
  const [matchPolicy, setMatchPolicy] = useState("");
  // The inline composer: editingIndex === null means the bottom "add" row,
  // otherwise we're rewriting the rule at that list position in place.
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [model, setModel] = useState<RuleModel>(EMPTY_RULE);
  const [policies, setPolicies] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [bulkPolicy, setBulkPolicy] = useState("");
  const [batchMode, setBatchMode] = useState(false);
  const [batchText, setBatchText] = useState("");

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  useEffect(() => {
    const all = initial
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l !== "");
    const nonMatch: string[] = [];
    let mp = "";
    for (const l of all) {
      if (isMatchLine(l)) mp = parseRule(l).policy; // last MATCH wins
      else nonMatch.push(l);
    }
    setLines(nonMatch);
    setMatchPolicy(mp);
    setSelected(new Set());
    setEditingIndex(null);
    setModel(EMPTY_RULE);
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

  // Reassemble full content (rules + MATCH fallback last) and save. MATCH is
  // appended only when a fallback policy is set, so it is always the final line.
  const persist = useCallback(
    async (nextLines: string[], nextMatch: string = matchPolicy) => {
      const m = nextMatch.trim();
      const content = (m ? [...nextLines, `MATCH,${m}`] : nextLines).join("\n");
      setLines(nextLines);
      setMatchPolicy(m);
      try {
        await api(`/api/profiles/${profileId}/rules`, {
          method: "PUT",
          body: JSON.stringify({ content }),
        });
        onSaved();
      } catch (e) {
        message.error((e as ApiError).message ?? t("common.saveFailed"));
      }
    },
    [matchPolicy, profileId, onSaved, t],
  );

  // Rule order is semantic (first match wins), so reordering directly changes
  // behavior. Drag uses real list indices, so it is only enabled when no search
  // filter is hiding rows (otherwise indices are ambiguous).
  function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const next = arrayMove(lines, Number(active.id), Number(over.id));
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
    if (!model.type.trim() || !model.policy.trim() || !model.payload.trim()) {
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

  function moveTo(index: number, to: "top" | "bottom") {
    const next = arrayMove(lines, index, to === "top" ? 0 : lines.length - 1);
    void persist(next);
  }

  // ---- Multi-select bulk actions (keyed by real list index) ----
  function toggleSelect(index: number, checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(index);
      else next.delete(index);
      return next;
    });
  }

  function bulkDelete() {
    void persist(lines.filter((_, i) => !selected.has(i)));
    setSelected(new Set());
  }

  function applyBulkPolicy() {
    const p = bulkPolicy.trim();
    if (!p) return;
    const next = lines.map((l, i) => {
      if (!selected.has(i) || isComment(l)) return l;
      return serializeRule({ ...parseRule(l), policy: p });
    });
    setSelected(new Set());
    setBulkPolicy("");
    void persist(next);
  }

  // ---- Batch (free-text) edit ----
  function enterBatch() {
    setBatchText(lines.join("\n"));
    setBatchMode(true);
    resetComposer();
    setSelected(new Set());
  }

  function saveBatch() {
    const next = batchText
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l !== "" && !isMatchLine(l)); // MATCH stays in its own control
    setBatchMode(false);
    void persist(next);
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
        .filter((l) => l !== "" && !isMatchLine(l) && !existing.has(l));
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

  const policyOptions = useMemo(
    () =>
      Array.from(
        new Set(
          [
            ...policies,
            ...nodes.map((n) => n.name),
            ...groups.map((g) => g.name),
            ...BUILTIN_POLICIES,
          ].filter((s) => s.trim() !== ""),
        ),
      ).map((value) => ({ value })),
    [policies, nodes, groups],
  );

  // Filtered view: keep each row's real index so edit/delete/select/move act on
  // the true position even while a search hides other rows.
  const q = search.trim().toLowerCase();
  const visible = lines
    .map((line, index) => ({ line, index }))
    .filter(({ line }) => q === "" || line.toLowerCase().includes(q));
  const dragEnabled = q === "" && editingIndex === null;

  const composer = (
    <RuleComposer
      model={model}
      onChange={setModel}
      onSubmit={submit}
      onCancel={editingIndex !== null ? resetComposer : undefined}
      submitLabel={editingIndex === null ? t("rules.add") : t("common.save")}
      ruleProviders={ruleProviders}
      policyOptions={policyOptions}
    />
  );

  return (
    <Card
      title={`${t("rules.title")} (${lines.length})`}
      extra={
        <Space>
          <Button onClick={() => (batchMode ? setBatchMode(false) : enterBatch())}>
            {batchMode ? t("common.cancel") : t("rules.batchEdit")}
          </Button>
          <Popconfirm title={t("rules.importConfirm")} onConfirm={importProviderRules}>
            <Button loading={importing}>{t("rules.importProvider")}</Button>
          </Popconfirm>
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

        {batchMode ? (
          <>
            <Typography.Text type="secondary">{t("rules.batchHint")}</Typography.Text>
            <Input.TextArea
              value={batchText}
              onChange={(e) => setBatchText(e.target.value)}
              autoSize={{ minRows: 8, maxRows: 24 }}
              spellCheck={false}
              style={{ fontFamily: "monospace" }}
            />
            <Space>
              <Button type="primary" onClick={saveBatch}>
                {t("common.save")}
              </Button>
              <Button onClick={() => setBatchMode(false)}>{t("common.cancel")}</Button>
            </Space>
          </>
        ) : (
          <>
            {/* Fallback policy (MATCH) — always applied last, never reorderable. */}
            <Space wrap style={{ padding: "4px 0" }}>
              <Typography.Text strong>{t("rules.fallback")}</Typography.Text>
              <AutoComplete
                style={{ width: 220 }}
                options={policyOptions}
                value={matchPolicy}
                onChange={(v) => setMatchPolicy(v)}
                onBlur={() => void persist(lines, matchPolicy)}
                placeholder={t("rules.fallbackNone")}
                filterOption={(input, opt) =>
                  String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
                }
              />
              <Typography.Text type="secondary">{t("rules.fallbackHint")}</Typography.Text>
            </Space>

            {/* Add composer (only in append mode; inline edit renders at its row). */}
            {editingIndex === null && composer}

            {/* Search + bulk action bar. */}
            {lines.length > 0 && (
              <Space wrap style={{ width: "100%" }}>
                <Input.Search
                  allowClear
                  placeholder={t("rules.search")}
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  style={{ width: 240 }}
                />
                {selected.size > 0 && (
                  <>
                    <Typography.Text>
                      {t("rules.selectedCount", { count: selected.size })}
                    </Typography.Text>
                    <AutoComplete
                      style={{ width: 180 }}
                      options={policyOptions}
                      value={bulkPolicy}
                      onChange={setBulkPolicy}
                      placeholder={t("rules.bulkPolicy")}
                    />
                    <Button onClick={applyBulkPolicy} disabled={!bulkPolicy.trim()}>
                      {t("rules.bulkApply")}
                    </Button>
                    <Popconfirm title={t("rules.deleteConfirm")} onConfirm={bulkDelete}>
                      <Button danger>{t("rules.bulkDelete")}</Button>
                    </Popconfirm>
                    <Button type="link" onClick={() => setSelected(new Set())}>
                      {t("rules.clearSelection")}
                    </Button>
                  </>
                )}
              </Space>
            )}

            {lines.length === 0 ? (
              <Empty description={t("rules.empty")} />
            ) : (
              <>
                <Typography.Text type="secondary">
                  {dragEnabled ? t("rules.dragHint") : t("rules.dragDisabledSearch")}
                </Typography.Text>
                <DndContext
                  sensors={sensors}
                  collisionDetection={closestCenter}
                  onDragEnd={onDragEnd}
                >
                  <SortableContext
                    items={visible.map(({ index }) => String(index))}
                    strategy={verticalListSortingStrategy}
                  >
                    <List>
                      {visible.map(({ line, index }) =>
                        index === editingIndex ? (
                          <List.Item key={index} style={{ background: "rgba(22,119,255,0.08)" }}>
                            {composer}
                          </List.Item>
                        ) : (
                          <SortableRuleItem
                            key={index}
                            id={String(index)}
                            line={line}
                            checked={selected.has(index)}
                            dragEnabled={dragEnabled}
                            onToggle={(c) => toggleSelect(index, c)}
                            onEdit={() => startEdit(index)}
                            onRemove={() => remove(index)}
                            onMoveTop={() => moveTo(index, "top")}
                            onMoveBottom={() => moveTo(index, "bottom")}
                          />
                        ),
                      )}
                    </List>
                  </SortableContext>
                </DndContext>
              </>
            )}
          </>
        )}
      </Space>
    </Card>
  );
}

interface ComposerProps {
  model: RuleModel;
  onChange: (m: RuleModel) => void;
  onSubmit: () => void;
  onCancel?: () => void;
  submitLabel: string;
  ruleProviders: RuleProvider[];
  policyOptions: { value: string }[];
}

// The type · content · no-resolve · policy row, reused for both "add" and
// in-place edit. The content input adapts to the selected rule type (à la Clash
// Verge): RULE-SET picks a defined rule-set name, NETWORK picks tcp/udp,
// everything else is a free text field with a per-type example placeholder.
function RuleComposer({
  model,
  onChange,
  onSubmit,
  onCancel,
  submitLabel,
  ruleProviders,
  policyOptions,
}: ComposerProps) {
  const { t } = useTranslation();
  const upperType = model.type.trim().toUpperCase();
  const showNoResolve = IP_TYPES.has(upperType);

  function contentInput() {
    if (upperType === "RULE-SET") {
      return (
        <AutoComplete
          style={{ width: 220 }}
          options={ruleProviders.map((rp) => ({ value: rp.name }))}
          value={model.payload}
          onChange={(payload) => onChange({ ...model, payload })}
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
          onChange={(payload) => onChange({ ...model, payload })}
          placeholder="tcp / udp"
        />
      );
    }
    return (
      <Input
        style={{ width: 220 }}
        value={model.payload}
        onChange={(e) => onChange({ ...model, payload: e.target.value })}
        placeholder={RULE_EXAMPLES[upperType] ?? t("rules.payload")}
      />
    );
  }

  return (
    <div
      style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", padding: "8px 0" }}
    >
      <AutoComplete
        style={{ width: 200 }}
        options={RULE_TYPES.map((v) => ({ value: v }))}
        value={model.type}
        onChange={(type) => onChange({ ...model, type })}
        placeholder={t("rules.ruleType")}
        filterOption={(input, opt) =>
          (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
        }
      />
      {contentInput()}
      {showNoResolve && (
        <Space size={4}>
          <Switch
            size="small"
            checked={model.noResolve}
            onChange={(noResolve) => onChange({ ...model, noResolve })}
          />
          <Typography.Text type="secondary">no-resolve</Typography.Text>
        </Space>
      )}
      <AutoComplete
        style={{ width: 200 }}
        options={policyOptions}
        value={model.policy}
        onChange={(policy) => onChange({ ...model, policy })}
        placeholder={t("rules.policy")}
        filterOption={(input, opt) =>
          String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
        }
      />
      <Button type="primary" onClick={onSubmit}>
        {submitLabel}
      </Button>
      {onCancel && <Button onClick={onCancel}>{t("common.cancel")}</Button>}
    </div>
  );
}

interface RuleItemProps {
  id: string;
  line: string;
  checked: boolean;
  dragEnabled: boolean;
  onToggle: (checked: boolean) => void;
  onEdit: () => void;
  onRemove: () => void;
  onMoveTop: () => void;
  onMoveBottom: () => void;
}

function SortableRuleItem({
  id,
  line,
  checked,
  dragEnabled,
  onToggle,
  onEdit,
  onRemove,
  onMoveTop,
  onMoveBottom,
}: RuleItemProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "rgba(0,0,0,0.04)" : checked ? "rgba(22,119,255,0.06)" : undefined,
  };
  const comment = isComment(line);
  const r = parseRule(line);

  const actions = [
    <a key="top" onClick={onMoveTop}>
      {t("rules.moveTop")}
    </a>,
    <a key="bottom" onClick={onMoveBottom}>
      {t("rules.moveBottom")}
    </a>,
    ...(comment
      ? []
      : [
          <a key="edit" onClick={onEdit}>
            {t("basic.edit")}
          </a>,
        ]),
    <Popconfirm key="del" title={t("rules.deleteConfirm")} onConfirm={onRemove}>
      <a>{t("rules.delete")}</a>
    </Popconfirm>,
  ];

  return (
    <List.Item ref={setNodeRef} style={style} actions={actions}>
      <Space>
        <Checkbox checked={checked} onChange={(e) => onToggle(e.target.checked)} />
        <span
          {...attributes}
          {...listeners}
          style={{
            cursor: dragEnabled ? "grab" : "not-allowed",
            color: "#999",
            userSelect: "none",
            touchAction: "none",
            opacity: dragEnabled ? 1 : 0.3,
            pointerEvents: dragEnabled ? "auto" : "none",
          }}
          aria-label="drag handle"
        >
          ⋮⋮
        </span>
        {comment ? (
          <Typography.Text type="secondary">{line}</Typography.Text>
        ) : (
          <Space wrap>
            <Tag>{r.type}</Tag>
            <span>{r.payload}</span>
            <span style={{ color: "#999" }}>→</span>
            <Tag color="blue">{r.policy}</Tag>
            {r.noResolve && <Tag>no-resolve</Tag>}
          </Space>
        )}
      </Space>
    </List.Item>
  );
}
