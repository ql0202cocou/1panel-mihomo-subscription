import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Button, Card, QRCode, Space, Typography, message } from "antd";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import type { ProfileSummary } from "../types";

// Minimal detail view for the skeleton step: hosted-link header with copy and
// QR. The full configuration cards and editors come in the next step.
export default function ProfileDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const [profile, setProfile] = useState<ProfileSummary | null>(null);

  useEffect(() => {
    if (id) {
      void api<ProfileSummary>(`/api/profiles/${id}`).then(setProfile);
    }
  }, [id]);

  if (!profile) {
    return null;
  }

  async function copyLink() {
    if (!profile) return;
    await navigator.clipboard.writeText(profile.subscription_url);
    message.success(t("detail.copied"));
  }

  return (
    <Space direction="vertical" size="large" style={{ width: "100%" }}>
      <Space style={{ justifyContent: "space-between", width: "100%" }}>
        <Typography.Title level={3} style={{ margin: 0 }}>
          {profile.name}
        </Typography.Title>
        <Link to="/">{t("detail.back")}</Link>
      </Space>

      <Card title={t("detail.hostedLink")}>
        <Space direction="vertical" size="middle" style={{ width: "100%" }}>
          <Typography.Text copyable code>
            {profile.subscription_url}
          </Typography.Text>
          <Space align="start" size="large">
            <Button type="primary" onClick={copyLink}>
              {t("detail.copy")}
            </Button>
            <QRCode value={profile.subscription_url} size={128} />
          </Space>
        </Space>
      </Card>

      <Typography.Paragraph type="secondary">
        {t("detail.editingHint")}
      </Typography.Paragraph>
    </Space>
  );
}
