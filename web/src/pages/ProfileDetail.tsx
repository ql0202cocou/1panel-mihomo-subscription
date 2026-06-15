import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Form,
  Input,
  Modal,
  Popconfirm,
  QRCode,
  Select,
  Space,
  Switch,
  Typography,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../api";
import type { ProfileDetail as Detail, SourceType } from "../types";
import NodesCard from "./detail/NodesCard";
import GroupsCard from "./detail/GroupsCard";
import RulesCard from "./detail/RulesCard";

const SOURCE_TYPES: SourceType[] = ["mihomo", "clash", "surge", "loon"];

/// The latest modification time across the profile and its sub-resources.
function latestModification(d: Detail): string {
  const times = [d.updated_at];
  if (d.rules) times.push(d.rules.updated_at);
  d.nodes.forEach((n) => times.push(n.updated_at));
  d.groups.forEach((g) => times.push(g.updated_at));
  return times.reduce((a, b) => (a > b ? a : b));
}

export default function ProfileDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const [detail, setDetail] = useState<Detail | null>(null);
  const [generating, setGenerating] = useState(false);
  const [genErrors, setGenErrors] = useState<string[]>([]);

  const reload = useCallback(async () => {
    if (id) setDetail(await api<Detail>(`/api/profiles/${id}`));
  }, [id]);

  useEffect(() => {
    void reload();
  }, [reload]);

  if (!detail) return null;

  async function generate() {
    if (!id) return;
    setGenerating(true);
    setGenErrors([]);
    try {
      await api(`/api/profiles/${id}/generate`, { method: "POST" });
      message.success(t("detail.generateSuccess"));
      await reload();
    } catch (e) {
      const err = e as ApiError;
      if (err.details && err.details.length > 0) {
        setGenErrors(err.details);
      } else {
        message.error(err.message ?? t("detail.generateFailed"));
      }
    } finally {
      setGenerating(false);
    }
  }

  const gen = detail.last_generated_at;
  const dirty = !gen || Date.parse(latestModification(detail)) > Date.parse(gen);
  const nonRuleErrors = genErrors.filter((e) => !/rules line/.test(e));

  return (
    <Space direction="vertical" size="large" style={{ width: "100%" }}>
      <Space style={{ justifyContent: "space-between", width: "100%" }}>
        <Typography.Title level={3} style={{ margin: 0 }}>
          {detail.name}
        </Typography.Title>
        <Link to="/">{t("detail.back")}</Link>
      </Space>

      <HostedLink detail={detail} dirty={dirty} onReset={reload} />

      <BasicInfo detail={detail} onSaved={reload} />
      <SourceInfo detail={detail} onRefresh={generate} onSaved={reload} refreshing={generating} />
      <NodesCard
        profileId={detail.id}
        profileName={detail.name}
        nodes={detail.nodes}
        generatedAt={detail.last_generated_at}
        onChange={reload}
      />
      <GroupsCard
        profileId={detail.id}
        groups={detail.groups}
        nodes={detail.nodes}
        generatedAt={detail.last_generated_at}
        onChange={reload}
      />
      <RulesCard
        profileId={detail.id}
        initial={detail.rules?.content ?? ""}
        nodes={detail.nodes}
        groups={detail.groups}
        generatedAt={detail.last_generated_at}
        errors={genErrors}
        onSaved={reload}
      />
      <PreviewCard profileId={detail.id} />

      {nonRuleErrors.length > 0 && (
        <Alert
          type="error"
          showIcon
          message={t("detail.generateFailed")}
          description={
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {nonRuleErrors.map((e, i) => (
                <li key={i}>{e}</li>
              ))}
            </ul>
          }
        />
      )}

      <Button type="primary" size="large" block loading={generating} onClick={generate}>
        {generating ? t("detail.generating") : t("detail.generate")}
      </Button>
    </Space>
  );
}

function HostedLink({
  detail,
  dirty,
  onReset,
}: {
  detail: Detail;
  dirty: boolean;
  onReset: () => void;
}) {
  const { t } = useTranslation();

  async function resetToken() {
    await api(`/api/profiles/${detail.id}/reset-token`, { method: "POST" });
    message.success(t("detail.copied"));
    onReset();
  }

  return (
    <Card title={t("detail.hostedLink")}>
      <Space direction="vertical" size="middle" style={{ width: "100%" }}>
        {dirty && (
          <Alert
            type="warning"
            showIcon
            message={detail.last_generated_at ? t("detail.notGenerated") : t("detail.neverGenerated")}
          />
        )}
        <Typography.Text copyable code>
          {detail.subscription_url}
        </Typography.Text>
        <Space align="start" size="large">
          <QRCode value={detail.subscription_url} size={128} />
          <Popconfirm title={t("detail.resetTokenConfirm")} onConfirm={resetToken}>
            <Button danger>{t("detail.resetToken")}</Button>
          </Popconfirm>
        </Space>
      </Space>
    </Card>
  );
}

function BasicInfo({ detail, onSaved }: { detail: Detail; onSaved: () => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm();

  async function save(values: { name: string; enabled: boolean; source_type: SourceType }) {
    await api(`/api/profiles/${detail.id}`, {
      method: "PUT",
      body: JSON.stringify(values),
    });
    setOpen(false);
    onSaved();
  }

  return (
    <Card
      title={t("basic.title")}
      extra={
        <Button
          onClick={() => {
            form.setFieldsValue({
              name: detail.name,
              enabled: detail.enabled,
              source_type: detail.source_type,
            });
            setOpen(true);
          }}
        >
          {t("basic.edit")}
        </Button>
      }
    >
      <Descriptions column={1}>
        <Descriptions.Item label={t("basic.name")}>{detail.name}</Descriptions.Item>
        <Descriptions.Item label={t("basic.enabled")}>
          {detail.enabled ? "✓" : "—"}
        </Descriptions.Item>
        <Descriptions.Item label={t("basic.outputType")}>{detail.output_type}</Descriptions.Item>
      </Descriptions>

      <Modal title={t("basic.title")} open={open} onCancel={() => setOpen(false)} onOk={() => form.submit()}>
        <Form form={form} layout="vertical" onFinish={save}>
          <Form.Item name="name" label={t("basic.name")} rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="source_type" label={t("source.type")}>
            <Select options={SOURCE_TYPES.map((s) => ({ value: s, label: s }))} />
          </Form.Item>
          <Form.Item name="enabled" label={t("basic.enabled")} valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </Card>
  );
}

function SourceInfo({
  detail,
  onRefresh,
  onSaved,
  refreshing,
}: {
  detail: Detail;
  onRefresh: () => void;
  onSaved: () => void;
  refreshing: boolean;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");

  async function save() {
    await api(`/api/profiles/${detail.id}`, {
      method: "PUT",
      body: JSON.stringify({ source_url: url }),
    });
    setOpen(false);
    setUrl("");
    onSaved();
  }

  return (
    <Card
      title={t("source.title")}
      extra={
        <Space>
          <Button onClick={() => setOpen(true)}>{t("source.newUrl")}</Button>
          <Button type="primary" loading={refreshing} onClick={onRefresh}>
            {t("source.refresh")}
          </Button>
        </Space>
      }
    >
      <Descriptions column={1}>
        <Descriptions.Item label={t("source.type")}>{detail.source_type}</Descriptions.Item>
        <Descriptions.Item label={t("source.url")}>{detail.source_url_masked}</Descriptions.Item>
        <Descriptions.Item label={t("source.lastFetch")}>
          {detail.last_fetch_status ? `${detail.last_fetch_status} · ${detail.last_fetch_at}` : t("source.never")}
        </Descriptions.Item>
      </Descriptions>

      <Modal title={t("source.newUrl")} open={open} onCancel={() => setOpen(false)} onOk={save}>
        <Typography.Paragraph type="secondary">{t("source.urlHint")}</Typography.Paragraph>
        <Input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://..." />
      </Modal>
    </Card>
  );
}

function PreviewCard({ profileId }: { profileId: string }) {
  const { t } = useTranslation();
  const [yaml, setYaml] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const text = await api<string>(`/api/profiles/${profileId}/preview`);
      setYaml(text);
    } catch (e) {
      message.error((e as ApiError).message ?? t("detail.generateFailed"));
    } finally {
      setLoading(false);
    }
  }

  return (
    <Card
      title={t("preview.title")}
      extra={
        <Button onClick={load} loading={loading}>
          {t("preview.load")}
        </Button>
      }
    >
      {yaml ? (
        <pre style={{ maxHeight: 320, overflow: "auto", margin: 0 }}>{yaml}</pre>
      ) : (
        <Typography.Text type="secondary">{t("preview.load")}</Typography.Text>
      )}
    </Card>
  );
}
