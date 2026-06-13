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
  Select,
  Space,
  Tag,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type { CustomGroup, GroupType } from "../../types";

const GROUP_TYPES: GroupType[] = ["select", "url-test", "fallback", "load-balance", "relay"];

interface Props {
  profileId: string;
  groups: CustomGroup[];
  onChange: () => void;
}

export default function GroupsCard({ profileId, groups, onChange }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<CustomGroup | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [groupType, setGroupType] = useState<GroupType>("select");
  const [members, setMembers] = useState<string[]>([]);
  const [optionsText, setOptionsText] = useState("");

  function startAdd() {
    setEditing(null);
    setName("");
    setGroupType("select");
    setMembers([]);
    setOptionsText("");
    setOpen(true);
  }

  function startEdit(group: CustomGroup) {
    setEditing(group);
    setName(group.name);
    setGroupType(group.group_type);
    setMembers(group.members);
    setOptionsText(group.options ? JSON.stringify(group.options, null, 2) : "");
    setOpen(true);
  }

  async function save() {
    let options: unknown = null;
    if (optionsText.trim()) {
      try {
        options = JSON.parse(optionsText);
        if (typeof options !== "object" || options === null || Array.isArray(options)) {
          throw new Error();
        }
      } catch {
        message.error(t("groups.optionsInvalid"));
        return;
      }
    }
    const body = JSON.stringify({
      name,
      group_type: groupType,
      members,
      options,
      enabled: true,
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
      message.error((e as ApiError).message ?? "保存失败");
    }
  }

  async function remove(group: CustomGroup) {
    await api(`/api/profiles/${profileId}/groups/${group.id}`, { method: "DELETE" });
    onChange();
  }

  return (
    <Card
      title={`${t("groups.title")} (${groups.length})`}
      extra={<Button onClick={startAdd}>{t("groups.add")}</Button>}
    >
      {groups.length === 0 ? (
        <Empty description={t("groups.empty")} />
      ) : (
        <List
          dataSource={groups}
          renderItem={(group) => (
            <List.Item
              actions={[
                <a key="edit" onClick={() => startEdit(group)}>
                  {t("basic.edit")}
                </a>,
                <Popconfirm
                  key="del"
                  title={t("groups.deleteConfirm")}
                  onConfirm={() => remove(group)}
                >
                  <a>{t("groups.delete")}</a>
                </Popconfirm>,
              ]}
            >
              <Space>
                <span>{group.name}</span>
                <Tag>{group.group_type}</Tag>
                <span style={{ color: "#999" }}>{group.members.length} 成员</span>
              </Space>
            </List.Item>
          )}
        />
      )}

      <Modal
        title={editing ? t("groups.edit") : t("groups.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={620}
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
              tokenSeparators={[","]}
              style={{ width: "100%" }}
            />
          </Form.Item>
          <Form.Item label={t("groups.options")}>
            <Input.TextArea
              value={optionsText}
              onChange={(e) => setOptionsText(e.target.value)}
              rows={4}
              placeholder={'{ "url": "https://www.gstatic.com/generate_204", "interval": 300 }'}
            />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}
