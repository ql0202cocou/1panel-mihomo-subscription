import { useState } from "react";
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
import type { RuleProvider, RuleProviderBehavior, RuleProviderType } from "../../types";
import { AdvancedFields, FieldInput, splitAdvanced } from "./fields";
import { RP_BEHAVIORS, RP_TYPES, rpOptionFields, rpOptionKeys } from "./ruleProviderSchema";

interface Props {
  profileId: string;
  ruleProviders: RuleProvider[];
  onChange: () => void;
}

type Options = Record<string, unknown>;

export default function RuleProvidersCard({ profileId, ruleProviders, onChange }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<RuleProvider | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [providerType, setProviderType] = useState<RuleProviderType>("http");
  const [behavior, setBehavior] = useState<RuleProviderBehavior>("domain");
  const [options, setOptions] = useState<Options>({});

  function startAdd() {
    setEditing(null);
    setName("");
    setProviderType("http");
    setBehavior("domain");
    setOptions({});
    setOpen(true);
  }

  function startEdit(rp: RuleProvider) {
    setEditing(rp);
    setName(rp.name);
    setProviderType(rp.provider_type);
    setBehavior(rp.behavior);
    setOptions(rp.options ?? {});
    setOpen(true);
  }

  function setOption(key: string, v: unknown) {
    const next = { ...options };
    if (v === "" || v === undefined || v === null) delete next[key];
    else next[key] = v;
    setOptions(next);
  }

  function setAdvancedOptions(rows: [string, unknown][]) {
    const known = rpOptionKeys(providerType);
    const next: Options = {};
    for (const [k, v] of Object.entries(options)) if (known.has(k)) next[k] = v;
    for (const [k, v] of rows) next[k] = v;
    setOptions(next);
  }

  async function save() {
    if (!name.trim()) {
      message.error(t("ruleProviders.nameRequired"));
      return;
    }
    // Drop empty keys; send null when no options remain.
    const cleaned: Options = {};
    for (const [k, v] of Object.entries(options)) {
      if (k.trim() === "" || v === "" || v === undefined || v === null) continue;
      if (Array.isArray(v) && v.length === 0) continue;
      cleaned[k] = v;
    }
    const body = JSON.stringify({
      name: name.trim(),
      provider_type: providerType,
      behavior,
      options: Object.keys(cleaned).length ? cleaned : null,
      enabled: editing ? editing.enabled : true,
    });
    try {
      if (editing) {
        await api(`/api/profiles/${profileId}/rule-providers/${editing.id}`, { method: "PUT", body });
      } else {
        await api(`/api/profiles/${profileId}/rule-providers`, { method: "POST", body });
      }
      setOpen(false);
      onChange();
    } catch (e) {
      message.error((e as ApiError).message ?? t("common.saveFailed"));
    }
  }

  async function remove(rp: RuleProvider) {
    await api(`/api/profiles/${profileId}/rule-providers/${rp.id}`, { method: "DELETE" });
    onChange();
  }

  const optionFields = rpOptionFields(providerType);
  const advancedOptions = splitAdvanced(options, rpOptionKeys(providerType));
  const total = ruleProviders.length;

  return (
    <Card
      title={`${t("ruleProviders.title")} (${total})`}
      extra={<Button onClick={startAdd}>{t("ruleProviders.add")}</Button>}
    >
      <Typography.Paragraph type="secondary">{t("ruleProviders.hint")}</Typography.Paragraph>
      {total === 0 ? (
        <Empty description={t("ruleProviders.empty")} />
      ) : (
        <List>
          {ruleProviders.map((rp) => (
            <List.Item
              key={rp.id}
              actions={[
                <a key="edit" onClick={() => startEdit(rp)}>
                  {t("basic.edit")}
                </a>,
                <Popconfirm
                  key="del"
                  title={t("ruleProviders.deleteConfirm")}
                  onConfirm={() => remove(rp)}
                >
                  <a>{t("ruleProviders.delete")}</a>
                </Popconfirm>,
              ]}
            >
              <Space wrap>
                <span>{rp.name}</span>
                <Tag>{rp.provider_type}</Tag>
                <Tag color="blue">{rp.behavior}</Tag>
                {!rp.enabled && <Tag>{t("profiles.disabled")}</Tag>}
              </Space>
            </List.Item>
          ))}
        </List>
      )}

      <Modal
        title={editing ? t("ruleProviders.edit") : t("ruleProviders.add")}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={620}
        destroyOnClose
      >
        <Form layout="vertical">
          <Form.Item label={t("ruleProviders.name")} help={t("ruleProviders.nameHint")} required>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </Form.Item>
          <Form.Item label={t("ruleProviders.type")} required>
            <Select
              value={providerType}
              onChange={(v) => setProviderType(v)}
              options={RP_TYPES.map((v) => ({ value: v, label: v }))}
            />
          </Form.Item>
          <Form.Item label={t("ruleProviders.behavior")} required>
            <Select
              value={behavior}
              onChange={(v) => setBehavior(v)}
              options={RP_BEHAVIORS.map((v) => ({ value: v, label: v }))}
            />
          </Form.Item>

          {optionFields.length > 0 && (
            <Divider orientation="left" plain>
              {t("ruleProviders.options")}
            </Divider>
          )}
          {optionFields.map((def) => (
            <Form.Item key={def.key} label={t(`ruleProviderFields.${def.key}`, def.key)}>
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
