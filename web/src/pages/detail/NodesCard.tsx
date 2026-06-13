import { useState } from "react";
import {
  Button,
  Card,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Popconfirm,
  Space,
  Tag,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type { CustomNode } from "../../types";
import YamlEditor from "../../components/YamlEditor";

interface Props {
  profileId: string;
  nodes: CustomNode[];
  onChange: () => void;
}

export default function NodesCard({ profileId, nodes, onChange }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<CustomNode | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [nodeType, setNodeType] = useState("");
  const [content, setContent] = useState("");

  function startAdd() {
    setEditing(null);
    setName("");
    setNodeType("");
    setContent("");
    setOpen(true);
  }

  function startEdit(node: CustomNode) {
    setEditing(node);
    setName(node.name);
    setNodeType(node.node_type);
    setContent(node.content);
    setOpen(true);
  }

  async function save() {
    const body = JSON.stringify({ name, node_type: nodeType, content, enabled: true });
    try {
      if (editing) {
        await api(`/api/profiles/${profileId}/nodes/${editing.id}`, { method: "PUT", body });
      } else {
        await api(`/api/profiles/${profileId}/nodes`, { method: "POST", body });
      }
      setOpen(false);
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? "保存失败");
    }
  }

  async function remove(node: CustomNode) {
    await api(`/api/profiles/${profileId}/nodes/${node.id}`, { method: "DELETE" });
    onChange();
  }

  return (
    <Card
      title={`${t("nodes.title")} (${nodes.length})`}
      extra={<Button onClick={startAdd}>{t("nodes.add")}</Button>}
    >
      {nodes.length === 0 ? (
        <Empty description={t("nodes.empty")} />
      ) : (
        <List
          dataSource={nodes}
          renderItem={(node) => (
            <List.Item
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
                <Tag>{node.node_type}</Tag>
              </Space>
            </List.Item>
          )}
        />
      )}

      <Modal
        title={editing ? t("nodes.edit") : t("nodes.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={680}
      >
        <Form layout="vertical">
          <Form.Item label={t("nodes.name")} required>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </Form.Item>
          <Form.Item label={t("nodes.type")} required>
            <Input
              value={nodeType}
              onChange={(e) => setNodeType(e.target.value)}
              placeholder="ss / vmess / vless / trojan / hysteria2"
            />
          </Form.Item>
          <Form.Item label={t("nodes.content")} required>
            <YamlEditor value={content} onChange={setContent} height="200px" />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
