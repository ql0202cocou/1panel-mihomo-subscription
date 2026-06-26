import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Modal, message } from "antd";
import {
  HolderOutlined,
  LockOutlined,
  PlusOutlined,
  ArrowRightOutlined,
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
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type { CustomNode, ProxiesResponse, ProxyPreview } from "../../types";
import NodeForm, { modelToContent, nodeAddr, type NodeModel } from "./NodeForm";

interface Props {
  profileId: string;
  /** 机场名,用作机场块的标题。 */
  profileName: string;
  /** 全局自定义节点池快照(此处只读;编辑在「节点配置」页)。 */
  nodes: CustomNode[];
  generatedAt: string | null;
  onChange: () => void;
}

const EMPTY_MODEL: NodeModel = { name: "", type: "", fields: {} };
const DEFAULT_SECTIONS = ["provider", "custom"];

export default function NodesCard({ profileId, profileName, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<ProxyPreview[]>([]);
  const [generated, setGenerated] = useState(true);
  const [sectionOrder, setSectionOrder] = useState<string[]>(DEFAULT_SECTIONS);

  const [open, setOpen] = useState(false);
  const [model, setModel] = useState<NodeModel>(EMPTY_MODEL);
  const [formKey, setFormKey] = useState("new");

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const loadProviders = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setProviders(res.proxies);
      setGenerated(res.generated);
      setSectionOrder(
        res.node_section_order.length === 2 ? res.node_section_order : DEFAULT_SECTIONS,
      );
    } catch {
      // 非致命:拿不到机场预览不影响只读展示。
    }
  }, [profileId]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders, generatedAt]);

  const customNames = useMemo(() => new Set(nodes.map((n) => n.name)), [nodes]);
  const providerNodes = useMemo(
    () => providers.filter((p) => !customNames.has(p.name)),
    [providers, customNames],
  );

  async function onSectionDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const next = arrayMove(
      sectionOrder,
      sectionOrder.indexOf(String(active.id)),
      sectionOrder.indexOf(String(over.id)),
    );
    setSectionOrder(next);
    try {
      await api(`/api/profiles/${profileId}/node-section-order`, {
        method: "PUT",
        body: JSON.stringify({ order: next }),
      });
      message.success(t("nodes.orderSaved"));
    } catch (e) {
      setSectionOrder(sectionOrder);
      message.error((e as ApiError).message ?? t("nodes.orderSaveFailed"));
    } finally {
      void loadProviders();
    }
  }

  function startAdd() {
    setModel(EMPTY_MODEL);
    setFormKey(`new-${Date.now()}`);
    setOpen(true);
  }

  async function save() {
    if (!model.name.trim() || !model.type.trim()) {
      message.error(t("nodes.nameTypeRequired"));
      return;
    }
    try {
      await api(`/api/global-nodes`, {
        method: "POST",
        body: JSON.stringify({
          name: model.name.trim(),
          node_type: model.type.trim(),
          content: modelToContent(model),
          enabled: true,
        }),
      });
      setOpen(false);
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? t("common.saveFailed"));
    }
  }

  const total = providerNodes.length + nodes.length;

  return (
    <div className="dcard">
      <div className="dcard-head">
        <span className="dcard-title">
          {t("nodes.title")} <span className="row-sub">{t("nodes.groupCount", { count: total })}</span>
        </span>
      </div>

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onSectionDragEnd}>
        <SortableContext items={sectionOrder} strategy={verticalListSortingStrategy}>
          {sectionOrder.map((key) =>
            key === "provider" ? (
              <Section
                key="provider"
                id="provider"
                title={profileName || t("nodes.providerTitle")}
                sub={t("nodes.providerReadonly")}
                count={providerNodes.length}
              >
                {providerNodes.length === 0 ? (
                  <div className="empty-line">
                    {generated ? t("nodes.providerEmpty") : t("nodes.providerNotGenerated")}
                  </div>
                ) : (
                  providerNodes.map((p) => (
                    <div className="row" key={p.name}>
                      <span className="row-lock">
                        <LockOutlined />
                      </span>
                      <span className="row-name">{p.name}</span>
                      {p.type && <span className="tag-mono tag-proto">{p.type}</span>}
                    </div>
                  ))
                )}
              </Section>
            ) : (
              <Section
                key="custom"
                id="custom"
                title={t("nodes.customGroup")}
                sub={t("nodes.customReadonly")}
                count={nodes.length}
                extra={
                  <Button size="small" type="dashed" icon={<PlusOutlined />} onClick={startAdd}>
                    {t("nodes.add")}
                  </Button>
                }
              >
                {nodes.length === 0 ? (
                  <div className="empty-line">{t("nodes.customEmpty")}</div>
                ) : (
                  nodes.map((n) => (
                    <div className="row" key={n.name}>
                      <span className="row-lock">
                        <LockOutlined />
                      </span>
                      <span className="row-name">{n.name}</span>
                      {n.node_type && <span className="tag-mono tag-proto custom">{n.node_type}</span>}
                      <span className="tag-addr">{nodeAddr(n.content)}</span>
                    </div>
                  ))
                )}
              </Section>
            ),
          )}
        </SortableContext>
      </DndContext>

      <div className="dcard-note">
        <ArrowRightOutlined style={{ fontSize: 11, marginRight: 4 }} />
        {t("nodes.manageHint")} <Link to="/nodes">{t("nav.nodes")}</Link>
      </div>

      <Modal
        title={t("nodes.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={680}
        okText={t("common.save")}
        cancelText={t("common.cancel")}
        destroyOnClose
      >
        <NodeForm key={formKey} value={model} onChange={setModel} />
      </Modal>
    </div>
  );
}

interface SectionProps {
  id: string;
  title: string;
  sub: string;
  count: number;
  extra?: React.ReactNode;
  children: React.ReactNode;
}

/** 可拖拽的分块(机场 / 自定义)。块内的行只读。 */
function Section({ id, title, sub, count, extra, children }: SectionProps) {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    background: isDragging ? "var(--bg-subtle)" : undefined,
  };
  return (
    <div className="node-section" ref={setNodeRef} style={style}>
      <div className="node-section-head">
        <span
          className="row-grab"
          {...attributes}
          {...listeners}
          aria-label="drag section"
        >
          <HolderOutlined />
        </span>
        <span className="node-section-title">{title}</span>
        <span className="node-section-sub">
          {t("nodes.groupCount", { count })} · {sub}
        </span>
        <span style={{ flex: 1 }} />
        {extra}
      </div>
      <div className="node-section-body">{children}</div>
    </div>
  );
}
