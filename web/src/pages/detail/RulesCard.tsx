import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Alert,
  AutoComplete,
  Button,
  Card,
  Empty,
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

// Common Mihomo rule types (free text still allowed in the selector). `MATCH`
// (the fallback) is a normal rule here: it takes only a policy and is
// added/edited/reordered/deleted like any other rule (it should stay last to be
// effective, but that's left to the admin's ordering).
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
  // Fallback
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
  generatedAt,
  errors,
  onSaved,
}: Props) {
  const { t } = useTranslation();
  // Source of truth is the raw, non-empty rule lines (including the MATCH
  // fallback, which is just a normal rule here). Comments and uncommon rules
  // (e.g. logical AND/OR) are preserved verbatim until explicitly edited.
  const [lines, setLines] = useState<string[]>([]);
  // The add/edit modal: editingIndex === null means we're adding a new rule,
  // otherwise we're rewriting the rule at that list position.
  const [modalOpen, setModalOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [model, setModel] = useState<RuleModel>(EMPTY_RULE);
  const [policies, setPolicies] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  useEffect(() => {
    const all = initial
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l !== "");
    setLines(all);
    setModalOpen(false);
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

  // Save the current rule lines verbatim (MATCH is a normal line in the list).
  const persist = useCallback(
    async (nextLines: string[]) => {
      const content = nextLines.join("\n");
      setLines(nextLines);
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

  // Rule order is semantic (first match wins), so reordering directly changes
  // behavior. Drag uses real list indices.
  function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const next = arrayMove(lines, Number(active.id), Number(over.id));
    void persist(next);
    message.success(t("rules.orderSaved"));
  }

  function openAdd() {
    setEditingIndex(null);
    setModel(EMPTY_RULE);
    setModalOpen(true);
  }

  function openEdit(index: number) {
    setEditingIndex(index);
    setModel(parseRule(lines[index]));
    setModalOpen(true);
  }

  function closeModal() {
    setModalOpen(false);
    setEditingIndex(null);
    setModel(EMPTY_RULE);
  }

  // Add (editingIndex === null) or save (rewrite at editingIndex) the composed
  // rule from the modal, then close it.
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
    closeModal();
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

  return (
    <Card
      title={`${t("rules.title")} (${lines.length})`}
      extra={
        <Space>
          <Button type="primary" onClick={openAdd}>
            {t("rules.add")}
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

        {lines.length === 0 ? (
          <Empty description={t("rules.empty")} />
        ) : (
          <>
            <Typography.Text type="secondary">{t("rules.dragHint")}</Typography.Text>
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
              <SortableContext
                items={lines.map((_, index) => String(index))}
                strategy={verticalListSortingStrategy}
              >
                <List>
                  {lines.map((line, index) => (
                    <SortableRuleItem
                      key={index}
                      id={String(index)}
                      line={line}
                      onEdit={() => openEdit(index)}
                      onRemove={() => remove(index)}
                    />
                  ))}
                </List>
              </SortableContext>
            </DndContext>
          </>
        )}
      </Space>

      <Modal
        open={modalOpen}
        title={editingIndex === null ? t("rules.add") : t("rules.edit")}
        onOk={submit}
        onCancel={closeModal}
        okText={editingIndex === null ? t("rules.add") : t("common.save")}
        cancelText={t("common.cancel")}
        destroyOnClose
      >
        <RuleComposer model={model} onChange={setModel} policyOptions={policyOptions} />
      </Modal>
    </Card>
  );
}

interface ComposerProps {
  model: RuleModel;
  onChange: (m: RuleModel) => void;
  policyOptions: { value: string }[];
}

// The structured rule fields (type · content · no-resolve · policy), rendered
// vertically inside the add/edit modal. The content input adapts to the selected
// rule type (à la Clash Verge): NETWORK picks tcp/udp, RULE-SET is a free-typed
// rule-set name (referencing the provider's own rule-providers — we don't host
// custom ones), everything else is a free text field with a per-type example.
function RuleComposer({ model, onChange, policyOptions }: ComposerProps) {
  const { t } = useTranslation();
  const upperType = model.type.trim().toUpperCase();
  const isMatch = upperType === "MATCH";
  const showNoResolve = IP_TYPES.has(upperType);

  function contentInput() {
    if (upperType === "RULE-SET") {
      return (
        <Input
          style={{ width: "100%" }}
          value={model.payload}
          onChange={(e) => onChange({ ...model, payload: e.target.value })}
          placeholder={t("rules.ruleSetPayloadHint")}
        />
      );
    }
    if (upperType === "NETWORK") {
      return (
        <AutoComplete
          style={{ width: "100%" }}
          options={[{ value: "tcp" }, { value: "udp" }]}
          value={model.payload}
          onChange={(payload) => onChange({ ...model, payload })}
          placeholder="tcp / udp"
        />
      );
    }
    return (
      <Input
        style={{ width: "100%" }}
        value={model.payload}
        onChange={(e) => onChange({ ...model, payload: e.target.value })}
        placeholder={RULE_EXAMPLES[upperType] ?? t("rules.payload")}
      />
    );
  }

  return (
    <Space direction="vertical" style={{ width: "100%" }} size="middle">
      <div>
        <Typography.Text type="secondary">{t("rules.ruleType")}</Typography.Text>
        <AutoComplete
          style={{ width: "100%" }}
          options={RULE_TYPES.map((v) => ({ value: v }))}
          value={model.type}
          onChange={(type) => onChange({ ...model, type })}
          placeholder={t("rules.ruleType")}
          filterOption={(input, opt) =>
            (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      </div>
      {isMatch ? (
        <Typography.Text type="secondary">{t("rules.matchHint")}</Typography.Text>
      ) : (
        <div>
          <Typography.Text type="secondary">{t("rules.payload")}</Typography.Text>
          {contentInput()}
        </div>
      )}
      {showNoResolve && !isMatch && (
        <Space size={8}>
          <Switch
            size="small"
            checked={model.noResolve}
            onChange={(noResolve) => onChange({ ...model, noResolve })}
          />
          <Typography.Text type="secondary">{t("rules.noResolve")}</Typography.Text>
        </Space>
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
    </Space>
  );
}

interface RuleItemProps {
  id: string;
  line: string;
  onEdit: () => void;
  onRemove: () => void;
}

function SortableRuleItem({ id, line, onEdit, onRemove }: RuleItemProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "rgba(0,0,0,0.04)" : undefined,
  };
  const comment = isComment(line);
  const r = parseRule(line);

  const actions = [
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
        <span
          {...attributes}
          {...listeners}
          style={{
            cursor: "grab",
            color: "#999",
            userSelect: "none",
            touchAction: "none",
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
            {r.payload && <span>{r.payload}</span>}
            <span style={{ color: "#999" }}>→</span>
            <Tag color="blue">{r.policy}</Tag>
            {r.noResolve && <Tag>no-resolve</Tag>}
          </Space>
        )}
      </Space>
    </List.Item>
  );
}
