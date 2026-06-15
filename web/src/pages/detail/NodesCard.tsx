import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Button, Card, Empty, List, Modal, Popconfirm, Space, Tag, Typography, message } from "antd";
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
  /** Provider (airport) name — used as the provider group's title. */
  profileName: string;
  nodes: CustomNode[];
  /** Changes when the profile is (re)generated; triggers a provider-node refetch. */
  generatedAt: string | null;
  onChange: () => void;
}

const EMPTY_MODEL: NodeModel = { name: "", type: "", fields: {} };
const DEFAULT_SECTIONS = ["provider", "custom"];

/** Order custom nodes by the generated output's custom subset, then append any
 * not yet in the output (disabled / newly added) so they stay visible. */
function buildCustomOrder(providers: ProxyPreview[], nodes: CustomNode[]): CustomNode[] {
  const customNames = new Set(nodes.map((n) => n.name));
  const byName = new Map(nodes.map((n) => [n.name, n]));
  const seen = new Set<string>();
  const out: CustomNode[] = [];
  for (const p of providers) {
    if (customNames.has(p.name) && !seen.has(p.name)) {
      out.push(byName.get(p.name)!);
      seen.add(p.name);
    }
  }
  for (const n of nodes) {
    if (!seen.has(n.name)) {
      out.push(n);
      seen.add(n.name);
    }
  }
  return out;
}

/** Preserve the current on-screen order for surviving nodes (avoids a reload
 * clobbering an optimistic drag); append new, drop removed. */
function reconcileNodes(prev: CustomNode[], derived: CustomNode[]): CustomNode[] {
  if (prev.length === 0) return derived;
  const byName = new Map(derived.map((n) => [n.name, n]));
  const result: CustomNode[] = [];
  for (const n of prev) {
    const d = byName.get(n.name);
    if (d) {
      result.push(d);
      byName.delete(n.name);
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

export default function NodesCard({ profileId, profileName, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<ProxyPreview[]>([]);
  const [generated, setGenerated] = useState(true);
  const [sectionOrder, setSectionOrder] = useState<string[]>(DEFAULT_SECTIONS);
  const [customRows, setCustomRows] = useState<CustomNode[]>([]);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(["custom"]));

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
      setSectionOrder(res.node_section_order.length === 2 ? res.node_section_order : DEFAULT_SECTIONS);
    } catch {
      // Non-fatal: the card still works for custom nodes without the preview.
    }
  }, [profileId]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders, generatedAt]);

  const customNames = useMemo(() => new Set(nodes.map((n) => n.name)), [nodes]);
  // Provider nodes = output proxies that aren't custom, in upstream order.
  const providerNodes = useMemo(
    () => providers.filter((p) => !customNames.has(p.name)),
    [providers, customNames],
  );
  const derivedCustom = useMemo(() => buildCustomOrder(providers, nodes), [providers, nodes]);
  useEffect(() => {
    setCustomRows((prev) => reconcileNodes(prev, derivedCustom));
  }, [derivedCustom]);

  function toggle(key: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

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

  async function persist(path: string, order: string[], onError: () => void) {
    try {
      await api(`/api/profiles/${profileId}/${path}`, {
        method: "PUT",
        body: JSON.stringify({ order }),
      });
      message.success(t("nodes.orderSaved"));
    } catch (e) {
      onError();
      message.error((e as ApiError).message ?? t("nodes.orderSaveFailed"));
    } finally {
      void loadProviders();
    }
  }

  // Reorder the two blocks.
  async function onGroupDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const next = arrayMove(
      sectionOrder,
      sectionOrder.indexOf(String(active.id)),
      sectionOrder.indexOf(String(over.id)),
    );
    setSectionOrder(next);
    await persist("node-section-order", next, () => setSectionOrder(sectionOrder));
  }

  // Reorder custom nodes within the custom block.
  async function onCustomDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = customRows.findIndex((n) => n.name === active.id);
    const newIndex = customRows.findIndex((n) => n.name === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(customRows, oldIndex, newIndex);
    setCustomRows(next);
    await persist("node-order", next.map((n) => n.name), () => setCustomRows(customRows));
  }

  const total = providerNodes.length + customRows.length;

  return (
    <Card title={`${t("nodes.title")} (${total})`}>
      <Typography.Paragraph type="secondary">{t("nodes.dragHint")}</Typography.Paragraph>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onGroupDragEnd}>
        <SortableContext items={sectionOrder} strategy={verticalListSortingStrategy}>
          {sectionOrder.map((key) =>
            key === "provider" ? (
              <GroupPanel
                key="provider"
                id="provider"
                title={profileName}
                count={providerNodes.length}
                open={expanded.has("provider")}
                onToggle={() => toggle("provider")}
              >
                {providerNodes.length === 0 ? (
                  <Typography.Paragraph type="secondary" style={{ margin: 0 }}>
                    {generated ? t("nodes.providerEmpty") : t("nodes.providerNotGenerated")}
                  </Typography.Paragraph>
                ) : (
                  <>
                    <List size="small">
                      {providerNodes.map((p) => (
                        <List.Item key={p.name}>
                          <Space>
                            <span>{p.name}</span>
                            {p.type && <Tag>{p.type}</Tag>}
                          </Space>
                        </List.Item>
                      ))}
                    </List>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      {t("nodes.providerReadonly")}
                    </Typography.Text>
                  </>
                )}
              </GroupPanel>
            ) : (
              <GroupPanel
                key="custom"
                id="custom"
                title={t("nodes.customGroup")}
                count={customRows.length}
                open={expanded.has("custom")}
                onToggle={() => toggle("custom")}
                extra={
                  <Button size="small" onClick={startAdd}>
                    {t("nodes.add")}
                  </Button>
                }
              >
                {customRows.length === 0 ? (
                  <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t("nodes.customEmpty")} />
                ) : (
                  <DndContext
                    sensors={sensors}
                    collisionDetection={closestCenter}
                    onDragEnd={onCustomDragEnd}
                  >
                    <SortableContext
                      items={customRows.map((n) => n.name)}
                      strategy={verticalListSortingStrategy}
                    >
                      <List size="small">
                        {customRows.map((node) => (
                          <SortableNodeRow
                            key={node.name}
                            node={node}
                            onEdit={startEdit}
                            onRemove={remove}
                          />
                        ))}
                      </List>
                    </SortableContext>
                  </DndContext>
                )}
              </GroupPanel>
            ),
          )}
        </SortableContext>
      </DndContext>

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

interface GroupPanelProps {
  id: string;
  title: string;
  count: number;
  open: boolean;
  onToggle: () => void;
  extra?: ReactNode;
  children: ReactNode;
}

/** A draggable, collapsible group panel. The header carries the drag handle and
 * a clickable title that toggles the body. */
function GroupPanel({ id, title, count, open, onToggle, extra, children }: GroupPanelProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    border: "1px solid #f0f0f0",
    borderRadius: 8,
    marginBottom: 8,
    background: isDragging ? "rgba(0,0,0,0.02)" : "#fff",
  };
  return (
    <div ref={setNodeRef} style={style}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 12px" }}>
        <span
          {...attributes}
          {...listeners}
          style={{ cursor: "grab", color: "#999", userSelect: "none", touchAction: "none" }}
          aria-label="drag handle"
        >
          ⋮⋮
        </span>
        <a onClick={onToggle} style={{ flex: 1, color: "inherit" }}>
          <Space>
            <span style={{ color: "#999" }}>{open ? "▾" : "▸"}</span>
            <strong>{title}</strong>
            <span style={{ color: "#999", fontWeight: "normal" }}>
              {t("nodes.groupCount", { count })}
            </span>
          </Space>
        </a>
        {extra}
      </div>
      {open && <div style={{ padding: "0 12px 8px 32px" }}>{children}</div>}
    </div>
  );
}

interface RowProps {
  node: CustomNode;
  onEdit: (node: CustomNode) => void;
  onRemove: (node: CustomNode) => void;
}

function SortableNodeRow({ node, onEdit, onRemove }: RowProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: node.name,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "rgba(0,0,0,0.04)" : undefined,
  };
  return (
    <List.Item
      ref={setNodeRef}
      style={style}
      actions={[
        <a key="edit" onClick={() => onEdit(node)}>
          {t("basic.edit")}
        </a>,
        <Popconfirm key="del" title={t("nodes.deleteConfirm")} onConfirm={() => onRemove(node)}>
          <a>{t("nodes.delete")}</a>
        </Popconfirm>,
      ]}
    >
      <Space>
        <span
          {...attributes}
          {...listeners}
          style={{ cursor: "grab", color: "#999", userSelect: "none", touchAction: "none" }}
          aria-label="drag handle"
        >
          ⋮⋮
        </span>
        <span>{node.name}</span>
        {node.node_type && <Tag>{node.node_type}</Tag>}
        {!node.enabled && <Tag>{t("profiles.disabled")}</Tag>}
      </Space>
    </List.Item>
  );
}
