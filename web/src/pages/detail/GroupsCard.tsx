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
  Typography,
  message,
} from "antd";
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

  // Custom groups are editable; provider groups already merged into the output
  // (matched by name) are dropped from the read-only section to avoid duplicates.
  const customNames = new Set(groups.map((g) => g.name));
  const providerOnly = providerGroups.filter((g) => !customNames.has(g.name));
  const total = groups.length + providerOnly.length;

  return (
    <Card
      title={`${t("groups.title")} (${total})`}
      extra={<Button onClick={startAdd}>{t("groups.add")}</Button>}
    >
      {!generated && (
        <Typography.Paragraph type="secondary">{t("groups.notGenerated")}</Typography.Paragraph>
      )}
      {total === 0 ? (
        <Empty description={t("groups.empty")} />
      ) : (
        <List>
          {groups.map((group) => (
            <List.Item
              key={`c-${group.id}`}
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
                <Tag color="blue">{t("groups.customTag")}</Tag>
                <span style={{ color: "#999" }}>
                  {t("groups.membersCount", { count: group.members.length })}
                </span>
                {!group.enabled && <Tag>{t("profiles.disabled")}</Tag>}
              </Space>
            </List.Item>
          ))}
          {providerOnly.map((g) => (
            <List.Item key={`p-${g.name}`}>
              <Space>
                <span>{g.name}</span>
                {g.type && <Tag>{g.type}</Tag>}
                <Tag>{t("groups.providerTag")}</Tag>
              </Space>
            </List.Item>
          ))}
        </List>
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
