import { useCallback, useEffect, useState } from "react";
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
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../../api";
import type { CustomGroup, CustomNode, GroupType, ProxiesResponse } from "../../types";
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

export default function GroupsCard({ profileId, groups, nodes, generatedAt, onChange }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<CustomGroup | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [groupType, setGroupType] = useState<GroupType>("select");
  const [members, setMembers] = useState<string[]>([]);
  const [options, setOptions] = useState<Options>({});

  // Provider proxies/groups from the latest generated output, for suggestions.
  const [providerProxies, setProviderProxies] = useState<string[]>([]);
  const [providerGroups, setProviderGroups] = useState<string[]>([]);

  const loadProviders = useCallback(async () => {
    try {
      const res = await api<ProxiesResponse>(`/api/profiles/${profileId}/proxies`);
      setProviderProxies(res.proxies.map((p) => p.name));
      setProviderGroups(res.groups);
    } catch {
      // Non-fatal: members can still be typed in by hand.
    }
  }, [profileId]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders, generatedAt]);

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
    ...providerGroups,
    ...nodes.map((n) => n.name),
    ...groups.filter((g) => g.id !== editing?.id).map((g) => g.name),
    ...BUILTIN_POLICIES,
  ]).map((value) => ({ value, label: value }));

  const optionFields = groupOptionFields(groupType);
  const advancedOptions = splitAdvanced(options, groupOptionKeys(groupType));

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
                <span style={{ color: "#999" }}>
                  {t("groups.membersCount", { count: group.members.length })}
                </span>
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

function dedupe(items: string[]): string[] {
  return Array.from(new Set(items.filter((s) => s.trim() !== "")));
}
