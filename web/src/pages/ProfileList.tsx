import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Button,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Select,
  Space,
  Tag,
  Typography,
  message,
} from "antd";
import { useTranslation } from "react-i18next";
import { api, type ApiError } from "../api";
import type { ProfileSummary, SourceType } from "../types";

const SOURCE_TYPES: SourceType[] = ["mihomo", "clash", "surge", "loon"];

export default function ProfileList() {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [form] = Form.useForm();

  async function load() {
    setLoading(true);
    try {
      setProfiles(await api<ProfileSummary[]>("/api/profiles"));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function onCreate(values: {
    name: string;
    source_type: SourceType;
    source_url: string;
  }) {
    try {
      await api<ProfileSummary>("/api/profiles", {
        method: "POST",
        body: JSON.stringify(values),
      });
      setCreating(false);
      form.resetFields();
      await load();
    } catch (e) {
      const err = e as ApiError;
      message.error(err.message ?? "创建失败");
    }
  }

  return (
    <Space direction="vertical" size="large" style={{ width: "100%" }}>
      <Space style={{ justifyContent: "space-between", width: "100%" }}>
        <Typography.Title level={3} style={{ margin: 0 }}>
          {t("profiles.title")}
        </Typography.Title>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t("profiles.create")}
        </Button>
      </Space>

      {!loading && profiles.length === 0 ? (
        <Empty description={t("profiles.empty")} />
      ) : (
        <List
          loading={loading}
          bordered
          dataSource={profiles}
          renderItem={(p) => (
            <List.Item
              actions={[
                <Link key="open" to={`/profiles/${p.id}`}>
                  {t("profiles.open")}
                </Link>,
              ]}
            >
              <List.Item.Meta
                title={p.name}
                description={
                  <Space>
                    <Tag>{p.source_type}</Tag>
                    {p.enabled ? (
                      <Tag color="green">{t("profiles.enabled")}</Tag>
                    ) : (
                      <Tag>{t("profiles.disabled")}</Tag>
                    )}
                    {p.last_fetch_status && (
                      <span>
                        {t("profiles.lastFetch")}: {p.last_fetch_status}
                      </span>
                    )}
                  </Space>
                }
              />
            </List.Item>
          )}
        />
      )}

      <Modal
        title={t("profiles.create")}
        open={creating}
        onCancel={() => setCreating(false)}
        onOk={() => form.submit()}
        okText={t("common.create")}
        cancelText={t("common.cancel")}
      >
        <Form form={form} layout="vertical" onFinish={onCreate}>
          <Form.Item name="name" label={t("profiles.name")} rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item
            name="source_type"
            label={t("profiles.sourceType")}
            initialValue="clash"
            rules={[{ required: true }]}
          >
            <Select options={SOURCE_TYPES.map((s) => ({ value: s, label: s }))} />
          </Form.Item>
          <Form.Item
            name="source_url"
            label={t("profiles.sourceUrl")}
            rules={[{ required: true }]}
          >
            <Input placeholder="https://..." />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}
