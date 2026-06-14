import { useCallback, useEffect, useState } from "react";
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

export default function NodesCard({ profileId, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<ProxyPreview[]>([]);
  const [generated, setGenerated] = useState(true);

  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<CustomNode | null>(null);
  const [model, setModel] = useState<NodeModel>(EMPTY_MODEL);
  const [formKey, setFormKey] = useState("new");

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

  // Custom nodes are editable; provider proxies already merged into the output
  // (matched by name) are dropped from the read-only section to avoid duplicates.
  const customNames = new Set(nodes.map((n) => n.name));
  const providerOnly = providers.filter((p) => !customNames.has(p.name));
  const total = nodes.length + providerOnly.length;

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
        <List>
          {nodes.map((node) => (
            <List.Item
              key={`c-${node.id}`}
              actions={[
                <a key="edit" onClick={() => startEdit(node)}>
                  {t("basic.edit")}
                </a>,
                <Popconfirm
                  key="del"
                  title={t("nodes.deleteConfirm")}
                  onConfirm={() => remove(node)}
                >
                  <a>{t("nodes.delete")}</a>
                </Popconfirm>,
              ]}
            >
              <Space>
                <span>{node.name}</span>
                {node.node_type && <Tag>{node.node_type}</Tag>}
                <Tag color="blue">{t("nodes.customTag")}</Tag>
                {!node.enabled && <Tag>{t("profiles.disabled")}</Tag>}
              </Space>
            </List.Item>
          ))}
          {providerOnly.map((p) => (
            <List.Item key={`p-${p.name}`}>
              <Space>
                <span>{p.name}</span>
                {p.type && <Tag>{p.type}</Tag>}
                <Tag>{t("nodes.providerTag")}</Tag>
              </Space>
            </List.Item>
          ))}
        </List>
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
