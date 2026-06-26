import { useCallback, useEffect, useMemo, useState } from "react";
import { AutoComplete, Button, Input, Modal, Popconfirm, Switch, Typography, message } from "antd";
import {
  ArrowRightOutlined,
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  LockOutlined,
  PlusOutlined,
} from "@ant-design/icons";
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
import type { CustomGroup, CustomNode, ProviderRulesResponse, ProxiesResponse } from "../../types";
import { BUILTIN_POLICIES } from "./groupSchema";

interface Props {
  profileId: string;
  initial: string;
  nodes: CustomNode[];
  groups: CustomGroup[];
  generatedAt: string | null;
  errors: string[];
  onSaved: () => void;
}

// 全部 Mihomo 规则类型,按分类分组以便选择器可浏览;仍允许自由输入未列出的类型。
const RULE_TYPE_GROUPS: { key: string; types: string[] }[] = [
  { key: "domain", types: ["DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-REGEX", "GEOSITE"] },
  {
    key: "ip",
    types: ["IP-CIDR", "IP-CIDR6", "IP-SUFFIX", "IP-ASN", "GEOIP", "SRC-GEOIP", "SRC-IP-ASN", "SRC-IP-CIDR", "SRC-IP-SUFFIX"],
  },
  { key: "port", types: ["DST-PORT", "SRC-PORT", "IN-PORT"] },
  { key: "process", types: ["PROCESS-NAME", "PROCESS-PATH", "PROCESS-NAME-REGEX", "PROCESS-PATH-REGEX"] },
  { key: "inbound", types: ["IN-TYPE", "IN-USER", "IN-NAME", "UID", "NETWORK", "DSCP", "RULE-SET"] },
  { key: "logical", types: ["AND", "OR", "NOT", "SUB-RULE"] },
  { key: "fallback", types: ["MATCH"] },
];

const NETWORK_OPTIONS = ["tcp", "udp"];
const IN_TYPE_OPTIONS = ["HTTP", "HTTPS", "SOCKS", "SOCKS4", "SOCKS5", "MIXED", "REDIR", "TPROXY", "TUN"];
const LOGICAL_TYPES = new Set(["AND", "OR", "NOT", "SUB-RULE"]);
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
  "DST-PORT": "443",
  "SRC-PORT": "8080",
  "IN-PORT": "7890",
  "PROCESS-NAME": "curl",
  "PROCESS-PATH": "/usr/bin/curl",
  "IN-TYPE": "SOCKS5",
  UID: "1000",
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

// 编辑目标:null = 新增;数字 = 编辑该条非 MATCH 规则;"match" = 编辑钉底的 MATCH。
type EditTarget = number | "match" | null;

export default function RulesCard({ profileId, initial, nodes, groups, generatedAt, errors, onSaved }: Props) {
  const { t } = useTranslation();
  // MATCH 钉在底部(锁定)。`rules` 按序保存其余所有行;`matchLine` 保存唯一的 MATCH 兜底
  // (或 null)。
  const [rules, setRules] = useState<string[]>([]);
  const [matchLine, setMatchLine] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<EditTarget>(null);
  const [model, setModel] = useState<RuleModel>(EMPTY_RULE);
  const [policies, setPolicies] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  useEffect(() => {
    const all = initial.split("\n").map((l) => l.trim()).filter((l) => l !== "");
    // 最后一条 MATCH 作为兜底,钉底保留;其余行原样留在列表。
    let match: string | null = null;
    const rest: string[] = [];
    for (const line of all) {
      if (isMatchLine(line)) match = line;
      else rest.push(line);
    }
    setRules(rest);
    setMatchLine(match);
    setModalOpen(false);
    setEditing(null);
    setModel(EMPTY_RULE);
  }, [initial]);

  const loadPolicies = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setPolicies([...res.proxies.map((p) => p.name), ...res.groups.map((g) => g.name)]);
    } catch {
      // 策略仍可手动输入
    }
  }, [profileId]);

  useEffect(() => {
    void loadPolicies();
  }, [loadPolicies, generatedAt]);

  // 保存规则 + 钉底的 MATCH(始终在最后)。
  const persist = useCallback(
    async (nextRules: string[], nextMatch: string | null) => {
      setRules(nextRules);
      setMatchLine(nextMatch);
      const content = [...nextRules, ...(nextMatch ? [nextMatch] : [])].join("\n");
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
    [profileId, onSaved, t],
  );

  function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const next = arrayMove(rules, Number(active.id), Number(over.id));
    void persist(next, matchLine);
    message.success(t("rules.orderSaved"));
  }

  function openAdd() {
    setEditing(null);
    setModel(EMPTY_RULE);
    setModalOpen(true);
  }
  function openEdit(index: number) {
    setEditing(index);
    setModel(parseRule(rules[index]));
    setModalOpen(true);
  }
  function openEditMatch() {
    setEditing("match");
    setModel(matchLine ? parseRule(matchLine) : { ...EMPTY_RULE, type: "MATCH" });
    setModalOpen(true);
  }
  function closeModal() {
    setModalOpen(false);
    setEditing(null);
    setModel(EMPTY_RULE);
  }

  function submit() {
    const isMatch = model.type.trim().toUpperCase() === "MATCH";
    if (!model.type.trim() || !model.policy.trim() || (!isMatch && !model.payload.trim())) {
      message.error(t("rules.incomplete"));
      return;
    }
    const line = serializeRule(model);
    closeModal();
    if (isMatch) {
      // 任何 MATCH(新增或编辑)都成为唯一的钉底兜底。
      void persist(rules, line);
      return;
    }
    if (editing === "match") {
      // 在编辑钉底行时把类型改成了非 MATCH:将其落入列表,并清空钉底的 MATCH。
      void persist([...rules, line], null);
      return;
    }
    const next = editing === null ? [...rules, line] : rules.map((l, i) => (i === editing ? line : l));
    void persist(next, matchLine);
  }

  function remove(index: number) {
    void persist(rules.filter((_, i) => i !== index), matchLine);
  }

  async function importProviderRules() {
    setImporting(true);
    try {
      const res = await api<ProviderRulesResponse>(`/api/profiles/${profileId}/provider-rules`);
      const existing = new Set([...rules, ...(matchLine ? [matchLine] : [])]);
      const incoming = res.rules
        .map((l) => l.trim())
        .filter((l) => l !== "" && !isMatchLine(l) && !existing.has(l));
      if (incoming.length === 0) {
        message.info(t("rules.importNone"));
        return;
      }
      await persist([...rules, ...incoming], matchLine);
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
          [...policies, ...nodes.map((n) => n.name), ...groups.map((g) => g.name), ...BUILTIN_POLICIES].filter(
            (s) => s.trim() !== "",
          ),
        ),
      ).map((value) => ({ value })),
    [policies, nodes, groups],
  );

  const ruleErrors = errors.filter((e) => /rules line/.test(e));
  const total = rules.length + (matchLine ? 1 : 0);

  return (
    <div className="dcard">
      <div className="dcard-head">
        <span className="dcard-title">
          {t("rules.title")} <span className="row-sub">{t("rules.count", { count: total })}</span>
        </span>
        <div className="dcard-actions">
          <Popconfirm title={t("rules.importConfirm")} onConfirm={importProviderRules}>
            <Button loading={importing}>{t("rules.importProvider")}</Button>
          </Popconfirm>
          <Button type="primary" icon={<PlusOutlined />} onClick={openAdd}>
            {t("rules.add")}
          </Button>
        </div>
      </div>

      {ruleErrors.length > 0 && (
        <div className="warn-banner">
          {t("rules.invalid")}
          {ruleErrors.map((e, i) => (
            <div key={i}>{e}</div>
          ))}
        </div>
      )}

      {rules.length === 0 && !matchLine ? (
        <div className="empty-line">{t("rules.empty")}</div>
      ) : (
        <>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <SortableContext
              items={rules.map((_, i) => String(i))}
              strategy={verticalListSortingStrategy}
            >
              {rules.map((line, index) => (
                <SortableRuleRow
                  key={index}
                  id={String(index)}
                  line={line}
                  onEdit={() => openEdit(index)}
                  onRemove={() => remove(index)}
                />
              ))}
            </SortableContext>
          </DndContext>
          {matchLine && <MatchRow line={matchLine} onEdit={openEditMatch} />}
        </>
      )}
      <div className="dcard-note">{t("rules.dragHint")}</div>

      <Modal
        open={modalOpen}
        title={editing === null ? t("rules.add") : t("rules.edit")}
        onOk={submit}
        onCancel={closeModal}
        okText={editing === null ? t("rules.add") : t("common.save")}
        cancelText={t("common.cancel")}
        destroyOnClose
      >
        <RuleComposer model={model} onChange={setModel} policyOptions={policyOptions} />
      </Modal>
    </div>
  );
}

function SortableRuleRow({
  id,
  line,
  onEdit,
  onRemove,
}: {
  id: string;
  line: string;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "var(--bg-subtle)" : undefined,
  };
  const comment = isComment(line);
  const r = parseRule(line);
  return (
    <div className="row" ref={setNodeRef} style={style}>
      <span className="row-grab" {...attributes} {...listeners} aria-label="drag">
        <HolderOutlined />
      </span>
      {comment ? (
        <span className="rule-content" style={{ color: "var(--text-4)" }}>
          {line}
        </span>
      ) : (
        <>
          <span className="tag-mono tag-type">{r.type}</span>
          <span className="rule-content">{r.payload}</span>
          {r.noResolve && <span className="tag-mono">no-resolve</span>}
          <ArrowRightOutlined className="rule-arrow" style={{ fontSize: 11 }} />
          <span className="tag-mono tag-policy">{r.policy}</span>
        </>
      )}
      <span className="row-actions">
        {!comment && (
          <button className="icon-btn" onClick={onEdit} aria-label={t("basic.edit")}>
            <EditOutlined />
          </button>
        )}
        <Popconfirm title={t("rules.deleteConfirm")} onConfirm={onRemove}>
          <button className="icon-btn danger" aria-label={t("rules.delete")}>
            <DeleteOutlined />
          </button>
        </Popconfirm>
      </span>
    </div>
  );
}

// 钉底的兜底行:锁在底部,仅可编辑(不可拖拽、不可删除)。
function MatchRow({ line, onEdit }: { line: string; onEdit: () => void }) {
  const { t } = useTranslation();
  const r = parseRule(line);
  return (
    <div className="row row-match">
      <span className="row-lock">
        <LockOutlined />
      </span>
      <span className="tag-mono tag-type">MATCH</span>
      <span className="rule-content">{t("rules.fallbackNote")}</span>
      <ArrowRightOutlined className="rule-arrow" style={{ fontSize: 11 }} />
      <span className="tag-mono tag-policy">{r.policy}</span>
      <span className="row-actions">
        <button className="icon-btn" onClick={onEdit} aria-label={t("basic.edit")}>
          <EditOutlined />
        </button>
      </span>
    </div>
  );
}

interface ComposerProps {
  model: RuleModel;
  onChange: (m: RuleModel) => void;
  policyOptions: { value: string }[];
}

function RuleComposer({ model, onChange, policyOptions }: ComposerProps) {
  const { t } = useTranslation();
  const upperType = model.type.trim().toUpperCase();
  const isMatch = upperType === "MATCH";
  const isLogical = LOGICAL_TYPES.has(upperType);
  const showNoResolve = !isMatch;

  const typeOptions = RULE_TYPE_GROUPS.map((g) => ({
    label: t(`rules.typeGroups.${g.key}`),
    options: g.types.map((v) => ({ value: v })),
  }));

  function contentBlock() {
    if (upperType === "NETWORK") {
      return {
        label: t("rules.networkLabel"),
        input: (
          <AutoComplete
            style={{ width: "100%" }}
            options={NETWORK_OPTIONS.map((value) => ({ value }))}
            value={model.payload}
            onChange={(payload) => onChange({ ...model, payload })}
            placeholder="tcp / udp"
          />
        ),
      };
    }
    if (upperType === "IN-TYPE") {
      return {
        label: t("rules.inTypeLabel"),
        input: (
          <AutoComplete
            style={{ width: "100%" }}
            options={IN_TYPE_OPTIONS.map((value) => ({ value }))}
            value={model.payload}
            onChange={(payload) => onChange({ ...model, payload })}
            placeholder="HTTP / SOCKS5 / TUN ..."
            filterOption={(input, opt) =>
              String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
            }
          />
        ),
      };
    }
    if (upperType === "RULE-SET") {
      return {
        label: t("rules.ruleSetLabel"),
        input: (
          <Input
            style={{ width: "100%" }}
            value={model.payload}
            onChange={(e) => onChange({ ...model, payload: e.target.value })}
            placeholder={t("rules.ruleSetPayloadHint")}
          />
        ),
      };
    }
    if (isLogical) {
      return {
        label: t("rules.payload"),
        hint: t("rules.logicalHint"),
        input: (
          <Input.TextArea
            style={{ width: "100%" }}
            autoSize={{ minRows: 2, maxRows: 6 }}
            value={model.payload}
            onChange={(e) => onChange({ ...model, payload: e.target.value })}
            placeholder={RULE_EXAMPLES[upperType] ?? "(DOMAIN,example.com),(NETWORK,tcp)"}
          />
        ),
      };
    }
    return {
      label: t("rules.payload"),
      input: (
        <Input
          style={{ width: "100%" }}
          value={model.payload}
          onChange={(e) => onChange({ ...model, payload: e.target.value })}
          placeholder={RULE_EXAMPLES[upperType] ?? t("rules.payload")}
        />
      ),
    };
  }

  const content = contentBlock();

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
      <div>
        <Typography.Text type="secondary">{t("rules.ruleType")}</Typography.Text>
        <AutoComplete
          style={{ width: "100%" }}
          options={typeOptions}
          value={model.type}
          onChange={(type) => onChange({ ...model, type })}
          placeholder={t("rules.ruleType")}
          filterOption={(input, opt) =>
            String((opt as { value?: string })?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      </div>
      {isMatch ? (
        <Typography.Text type="secondary">{t("rules.matchHint")}</Typography.Text>
      ) : (
        <div>
          <Typography.Text type="secondary">{content.label}</Typography.Text>
          {content.input}
          {content.hint && (
            <Typography.Text type="secondary" style={{ display: "block", marginTop: 4 }}>
              {content.hint}
            </Typography.Text>
          )}
        </div>
      )}
      {showNoResolve && (
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Switch
            size="small"
            checked={model.noResolve}
            onChange={(noResolve) => onChange({ ...model, noResolve })}
          />
          <Typography.Text type="secondary">{t("rules.noResolve")}</Typography.Text>
        </div>
      )}
      <div>
        <Typography.Text type="secondary">{t("rules.policy")}</Typography.Text>
        <AutoComplete
          style={{ width: "100%" }}
          options={policyOptions}
          value={model.policy}
          onChange={(policy) => onChange({ ...model, policy })}
          placeholder={t("rules.policy")}
          filterOption={(input, opt) =>
            String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      </div>
    </div>
  );
}
