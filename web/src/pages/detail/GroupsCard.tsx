import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  Divider,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Popconfirm,
  Select,
  Space,
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
import type { CustomGroup, CustomNode, GroupType, ProxiesResponse, ProxyPreview } from "../../types";
import { AdvancedFields, FieldInput, splitAdvanced } from "./fields";
import {
  BUILTIN_POLICIES,
  GROUP_TYPES,
  groupOptionFields,
  groupOptionKeys,
} from "./groupSchema";

interface Props {
  profileId: string;
  groups: CustomGroup[];
  nodes: CustomNode[];
  /** Changes when the profile is (re)generated; refreshes member suggestions. */
  generatedAt: string | null;
  onChange: () => void;
}

type Options = Record<string, unknown>;

/** One row in the unified, sortable group list. */
interface GroupRow {
  /** Stable dnd/React id — group names are unique within a profile. */
  name: string;
  type: string;
  /** The editable custom group, or null for a read-only provider group. */
  custom: CustomGroup | null;
}

/**
 * Merge the generated proxy-group list (provider + custom, already in saved
 * order) with the custom-group list into one ordered row list. Falls back to
 * the custom groups alone before anything is generated; disabled custom groups
 * (absent from the output) are appended so they stay editable.
 */
function buildRows(
  providerGroups: ProxyPreview[],
  groups: CustomGroup[],
  generated: boolean,
): GroupRow[] {
  const customByName = new Map(groups.map((g) => [g.name, g]));
  if (!generated || providerGroups.length === 0) {
    return groups.map((g) => ({ name: g.name, type: g.group_type, custom: g }));
  }
  const rows: GroupRow[] = providerGroups.map((g) => ({
    name: g.name,
    type: g.type,
    custom: customByName.get(g.name) ?? null,
  }));
  const seen = new Set(providerGroups.map((g) => g.name));
  for (const g of groups) {
    if (!seen.has(g.name)) rows.push({ name: g.name, type: g.group_type, custom: g });
  }
  return rows;
}

/**
 * Merge freshly derived rows into the current on-screen order: keep the existing
 * order (with refreshed data) for rows that still exist, append new ones, drop
 * removed ones. Preserves an optimistic drag order even when the re-derived
 * server list can't yet reflect it (the saved order is already persisted).
 */
function reconcileRows(prev: GroupRow[], derived: GroupRow[]): GroupRow[] {
  if (prev.length === 0) return derived;
  const byName = new Map(derived.map((r) => [r.name, r]));
  const result: GroupRow[] = [];
  for (const r of prev) {
    const d = byName.get(r.name);
    if (d) {
      result.push(d);
      byName.delete(r.name);
    }
  }
  for (const d of derived) {
    if (byName.has(d.name)) {
      result.push(d);
      byName.delete(d.name);
    }
  }
  return result;
}

export default function GroupsCard({ profileId, groups, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<CustomGroup | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [groupType, setGroupType] = useState<GroupType>("select");
  const [members, setMembers] = useState<string[]>([]);
  const [options, setOptions] = useState<Options>({});

  // Provider proxies/groups from the latest generated output, for the read-only
  // preview and for member suggestions.
  const [providerProxies, setProviderProxies] = useState<string[]>([]);
  const [providerGroups, setProviderGroups] = useState<ProxyPreview[]>([]);
  const [generated, setGenerated] = useState(true);
  const [rows, setRows] = useState<GroupRow[]>([]);
  const [importing, setImporting] = useState(false);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const loadProviders = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setProviderProxies(res.proxies.map((p) => p.name));
      setProviderGroups(res.groups);
      setGenerated(res.generated);
    } catch {
      // Non-fatal: members can still be typed in by hand.
    }
  }, [profileId]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders, generatedAt]);

  // Keep the sortable rows in sync with the latest server/props state, but
  // preserve the current order for surviving rows so an optimistic drag isn't
  // clobbered by a reload that can't yet reflect it (see reconcileRows).
  const derived = useMemo(
    () => buildRows(providerGroups, groups, generated),
    [providerGroups, groups, generated],
  );
  useEffect(() => {
    setRows((prev) => reconcileRows(prev, derived));
  }, [derived]);

  async function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = rows.findIndex((r) => r.name === active.id);
    const newIndex = rows.findIndex((r) => r.name === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(rows, oldIndex, newIndex);
    setRows(next); // optimistic
    try {
      await api(`/api/profiles/${profileId}/group-order`, {
        method: "PUT",
        body: JSON.stringify({ order: next.map((r) => r.name) }),
      });
      message.success(t("groups.orderSaved"));
    } catch (e) {
      message.error((e as ApiError).message ?? t("groups.orderSaveFailed"));
    } finally {
      // Reconcile with the server (confirms on success, reverts on failure).
      void loadProviders();
    }
  }

  // Import the airport's own proxy-groups as editable custom groups (the
  // converter otherwise replaces provider groups). Appends, skipping existing.
  async function importProviderGroups() {
    setImporting(true);
    try {
      const res = await api<{ imported: number; skipped: number }>(
        `/api/profiles/${profileId}/import-provider-groups`,
        { method: "POST" },
      );
      if (res.imported === 0) {
        message.info(t("groups.importNone"));
      } else {
        message.success(t("groups.imported", { count: res.imported }));
      }
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? t("groups.importFailed"));
    } finally {
      setImporting(false);
    }
  }

  function startAdd() {
    setEditing(null);
    setName("");
    setGroupType("select");
    setMembers([]);
    setOptions({});
    setOpen(true);
  }

  function startEdit(group: CustomGroup) {
    setEditing(group);
    setName(group.name);
    setGroupType(group.group_type);
    setMembers(group.members);
    setOptions(group.options ?? {});
    setOpen(true);
  }

  function setOption(key: string, v: unknown) {
    const next = { ...options };
    if (v === "" || v === undefined || v === null) delete next[key];
    else next[key] = v;
    setOptions(next);
  }

  function setAdvancedOptions(rows: [string, unknown][]) {
    const known = groupOptionKeys(groupType);
    const next: Options = {};
    for (const [k, v] of Object.entries(options)) if (known.has(k)) next[k] = v;
    for (const [k, v] of rows) next[k] = v;
    setOptions(next);
  }

  async function save() {
    if (!name.trim()) {
      message.error(t("groups.nameRequired"));
      return;
    }
    // Drop empty keys; send null when no options remain.
    const cleaned: Options = {};
    for (const [k, v] of Object.entries(options)) {
      if (k.trim() === "" || v === "" || v === undefined || v === null) continue;
      cleaned[k] = v;
    }
    const body = JSON.stringify({
      name: name.trim(),
      group_type: groupType,
      members,
      options: Object.keys(cleaned).length ? cleaned : null,
      enabled: editing ? editing.enabled : true,
    });
    try {
      if (editing) {
        await api(`/api/profiles/${profileId}/groups/${editing.id}`, { method: "PUT", body });
      } else {
        await api(`/api/profiles/${profileId}/groups`, { method: "POST", body });
      }
      setOpen(false);
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? t("common.saveFailed"));
    }
  }

  async function remove(group: CustomGroup) {
    await api(`/api/profiles/${profileId}/groups/${group.id}`, { method: "DELETE" });
    onChange();
  }

  // Suggestions: provider proxies/groups, custom nodes/groups (minus the group
  // being edited, which cannot reference itself) and built-in policies.
  const memberOptions = dedupe([
    ...providerProxies,
    ...providerGroups.map((g) => g.name),
    ...nodes.map((n) => n.name),
    ...groups.filter((g) => g.id !== editing?.id).map((g) => g.name),
    ...BUILTIN_POLICIES,
  ]).map((value) => ({ value, label: value }));

  const optionFields = groupOptionFields(groupType);
  const advancedOptions = splitAdvanced(options, groupOptionKeys(groupType));

  const total = rows.length;

  return (
    <Card
      title={`${t("groups.title")} (${total})`}
      extra={
        <Space>
          <Popconfirm title={t("groups.importConfirm")} onConfirm={importProviderGroups}>
            <Button loading={importing}>{t("groups.importProvider")}</Button>
          </Popconfirm>
          <Button onClick={startAdd}>{t("groups.add")}</Button>
        </Space>
      }
    >
      {!generated && (
        <Typography.Paragraph type="secondary">{t("groups.notGenerated")}</Typography.Paragraph>
      )}
      {total === 0 ? (
        <Empty description={t("groups.empty")} />
      ) : (
        <>
          <Typography.Paragraph type="secondary">{t("groups.dragHint")}</Typography.Paragraph>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <SortableContext
              items={rows.map((r) => r.name)}
              strategy={verticalListSortingStrategy}
            >
              <List>
                {rows.map((row) => (
                  <SortableGroupRow
                    key={row.name}
                    row={row}
                    onEdit={startEdit}
                    onRemove={remove}
                  />
                ))}
              </List>
            </SortableContext>
          </DndContext>
        </>
      )}

      <Modal
        title={editing ? t("groups.edit") : t("groups.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={620}
        destroyOnClose
      >
        <Form layout="vertical">
          <Form.Item label={t("groups.name")} required>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </Form.Item>
          <Form.Item label={t("groups.type")} required>
            <Select
              value={groupType}
              onChange={setGroupType}
              options={GROUP_TYPES.map((g) => ({ value: g, label: g }))}
            />
          </Form.Item>
          <Form.Item label={t("groups.members")} help={t("groups.membersHint")}>
            <Select
              mode="tags"
              value={members}
              onChange={setMembers}
              options={memberOptions}
              tokenSeparators={[","]}
              style={{ width: "100%" }}
              filterOption={(input, opt) =>
                String(opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
              }
            />
          </Form.Item>

          {optionFields.length > 0 && (
            <Divider orientation="left" plain>
              {t("groups.options")}
            </Divider>
          )}
          {optionFields.map((def) => (
            <Form.Item key={def.key} label={t(`groupFields.${def.key}`, def.key)}>
              <FieldInput
                def={def}
                value={options[def.key]}
                onChange={(v) => setOption(def.key, v)}
              />
            </Form.Item>
          ))}

          <AdvancedFields entries={advancedOptions} onChange={setAdvancedOptions} />
        </Form>
      </Modal>
    </Card>
  );
}

interface RowProps {
  row: GroupRow;
  onEdit: (group: CustomGroup) => void;
  onRemove: (group: CustomGroup) => void;
}

function SortableGroupRow({ row, onEdit, onRemove }: RowProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: row.name,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "rgba(0,0,0,0.04)" : undefined,
  };

  const { custom } = row;
  const actions = custom
    ? [
        <a key="edit" onClick={() => onEdit(custom)}>
          {t("basic.edit")}
        </a>,
        <Popconfirm
          key="del"
          title={t("groups.deleteConfirm")}
          onConfirm={() => onRemove(custom)}
        >
          <a>{t("groups.delete")}</a>
        </Popconfirm>,
      ]
    : undefined;

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
        <span>{row.name}</span>
        {row.type && <Tag>{row.type}</Tag>}
        {custom ? (
          <>
            <Tag color="blue">{t("groups.customTag")}</Tag>
            <span style={{ color: "#999" }}>
              {t("groups.membersCount", { count: custom.members.length })}
            </span>
            {!custom.enabled && <Tag>{t("profiles.disabled")}</Tag>}
          </>
        ) : (
          <Tag>{t("groups.providerTag")}</Tag>
        )}
      </Space>
    </List.Item>
  );
}

function dedupe(items: string[]): string[] {
  return Array.from(new Set(items.filter((s) => s.trim() !== "")));
}
