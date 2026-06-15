import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  Empty,
  List,
  Modal,
  Popconfirm,
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
import type { CustomNode, ProxiesResponse, ProxyPreview } from "../../types";
import NodeForm, { contentToModel, modelToContent, type NodeModel } from "./NodeForm";

interface Props {
  profileId: string;
  nodes: CustomNode[];
  /** Changes when the profile is (re)generated; triggers a provider-node refetch. */
  generatedAt: string | null;
  onChange: () => void;
}

const EMPTY_MODEL: NodeModel = { name: "", type: "", fields: {} };

/** One row in the unified, sortable node list. */
interface NodeRow {
  /** Stable dnd/React id — proxy names are unique within a profile. */
  name: string;
  type: string;
  /** The editable custom node, or null for a read-only provider node. */
  custom: CustomNode | null;
}

/**
 * Merge the generated proxy list (provider + custom, already in saved order)
 * with the custom-node list into one ordered row list. When nothing has been
 * generated yet, falls back to the custom nodes alone. Disabled custom nodes
 * (absent from the generated output) are appended so they stay editable.
 */
function buildRows(providers: ProxyPreview[], nodes: CustomNode[], generated: boolean): NodeRow[] {
  const customByName = new Map(nodes.map((n) => [n.name, n]));
  if (!generated || providers.length === 0) {
    return nodes.map((n) => ({ name: n.name, type: n.node_type, custom: n }));
  }
  const rows: NodeRow[] = providers.map((p) => ({
    name: p.name,
    type: p.type,
    custom: customByName.get(p.name) ?? null,
  }));
  const seen = new Set(providers.map((p) => p.name));
  for (const n of nodes) {
    if (!seen.has(n.name)) rows.push({ name: n.name, type: n.node_type, custom: n });
  }
  return rows;
}

/**
 * Merge freshly derived rows into the current on-screen order: keep the existing
 * order (with refreshed data) for rows that still exist, append genuinely new
 * ones, drop removed ones. This preserves an optimistic drag order even when the
 * re-derived server list can't yet reflect it (e.g. a not-yet-generated profile,
 * or a custom node added but not regenerated) — the saved order is already
 * persisted, so this only fixes the visual snap-back.
 */
function reconcileRows(prev: NodeRow[], derived: NodeRow[]): NodeRow[] {
  if (prev.length === 0) return derived;
  const byName = new Map(derived.map((r) => [r.name, r]));
  const result: NodeRow[] = [];
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

export default function NodesCard({ profileId, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<ProxyPreview[]>([]);
  const [generated, setGenerated] = useState(true);
  const [rows, setRows] = useState<NodeRow[]>([]);

  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<CustomNode | null>(null);
  const [model, setModel] = useState<NodeModel>(EMPTY_MODEL);
  const [formKey, setFormKey] = useState("new");

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const loadProviders = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setProviders(res.proxies);
      setGenerated(res.generated);
    } catch {
      // Non-fatal: the card still works for custom nodes without the preview.
    }
  }, [profileId]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders, generatedAt]);

  // Keep the sortable rows in sync with the latest server/props state, but
  // preserve the current order for surviving rows so an optimistic drag isn't
  // clobbered by a reload that can't yet reflect it (see reconcileRows).
  const derived = useMemo(
    () => buildRows(providers, nodes, generated),
    [providers, nodes, generated],
  );
  useEffect(() => {
    setRows((prev) => reconcileRows(prev, derived));
  }, [derived]);

  function startAdd() {
    setEditing(null);
    setModel(EMPTY_MODEL);
    setFormKey(`new-${Date.now()}`);
    setOpen(true);
  }

  function startEdit(node: CustomNode) {
    setEditing(node);
    setModel(contentToModel(node.content));
    setFormKey(node.id);
    setOpen(true);
  }

  async function save() {
    if (!model.name.trim() || !model.type.trim()) {
      message.error(t("nodes.nameTypeRequired"));
      return;
    }
    const body = JSON.stringify({
      name: model.name.trim(),
      node_type: model.type.trim(),
      content: modelToContent(model),
      enabled: editing ? editing.enabled : true,
    });
    try {
      if (editing) {
        await api(`/api/profiles/${profileId}/nodes/${editing.id}`, { method: "PUT", body });
      } else {
        await api(`/api/profiles/${profileId}/nodes`, { method: "POST", body });
      }
      setOpen(false);
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? t("common.saveFailed"));
    }
  }

  async function remove(node: CustomNode) {
    await api(`/api/profiles/${profileId}/nodes/${node.id}`, { method: "DELETE" });
    onChange();
  }

  async function onDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = rows.findIndex((r) => r.name === active.id);
    const newIndex = rows.findIndex((r) => r.name === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(rows, oldIndex, newIndex);
    setRows(next); // optimistic
    try {
      await api(`/api/profiles/${profileId}/node-order`, {
        method: "PUT",
        body: JSON.stringify({ order: next.map((r) => r.name) }),
      });
      message.success(t("nodes.orderSaved"));
    } catch (e) {
      message.error((e as ApiError).message ?? t("nodes.orderSaveFailed"));
    } finally {
      // Reconcile with the server (confirms on success, reverts on failure).
      void loadProviders();
    }
  }

  const total = rows.length;

  return (
    <Card
      title={`${t("nodes.title")} (${total})`}
      extra={<Button onClick={startAdd}>{t("nodes.add")}</Button>}
    >
      {!generated && (
        <Typography.Paragraph type="secondary">{t("nodes.notGenerated")}</Typography.Paragraph>
      )}
      {total === 0 ? (
        <Empty description={t("nodes.empty")} />
      ) : (
        <>
          <Typography.Paragraph type="secondary">{t("nodes.dragHint")}</Typography.Paragraph>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <SortableContext
              items={rows.map((r) => r.name)}
              strategy={verticalListSortingStrategy}
            >
              <List>
                {rows.map((row) => (
                  <SortableNodeRow
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
        title={editing ? t("nodes.edit") : t("nodes.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={680}
        destroyOnClose
      >
        <NodeForm key={formKey} value={model} onChange={setModel} />
      </Modal>
    </Card>
  );
}

interface RowProps {
  row: NodeRow;
  onEdit: (node: CustomNode) => void;
  onRemove: (node: CustomNode) => void;
}

function SortableNodeRow({ row, onEdit, onRemove }: RowProps) {
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
          title={t("nodes.deleteConfirm")}
          onConfirm={() => onRemove(custom)}
        >
          <a>{t("nodes.delete")}</a>
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
            <Tag color="blue">{t("nodes.customTag")}</Tag>
            {!custom.enabled && <Tag>{t("profiles.disabled")}</Tag>}
          </>
        ) : (
          <Tag>{t("nodes.providerTag")}</Tag>
        )}
      </Space>
    </List.Item>
  );
}
