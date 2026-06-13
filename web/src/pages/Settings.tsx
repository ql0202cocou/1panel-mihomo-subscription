import { useEffect, useState } from "react";
import { Button, Card, Input, Modal, Space, Typography, message } from "antd";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import type { Settings as SettingsData } from "../types";

export default function Settings() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [confirmText, setConfirmText] = useState("");

  async function load() {
    setSettings(await api<SettingsData>("/api/settings"));
  }

  useEffect(() => {
    void load();
  }, []);

  async function onReset() {
    const next = await api<SettingsData>("/api/settings/reset-public-path", {
      method: "POST",
    });
    setSettings(next);
    setConfirming(false);
    setConfirmText("");
    message.success(t("settings.publicPathPrefix") + ": " + next.public_path_prefix);
  }

  return (
    <Space direction="vertical" size="large" style={{ width: "100%" }}>
      <Typography.Title level={3} style={{ margin: 0 }}>
        {t("settings.title")}
      </Typography.Title>

      <Card title={t("settings.publicPathPrefix")}>
        <Space direction="vertical" style={{ width: "100%" }}>
          <Typography.Text code>{settings?.public_path_prefix}</Typography.Text>
          <Button danger onClick={() => setConfirming(true)}>
            {t("settings.resetPublicPath")}
          </Button>
        </Space>
      </Card>

      <Modal
        title={t("settings.resetPublicPath")}
        open={confirming}
        onCancel={() => setConfirming(false)}
        okButtonProps={{ danger: true, disabled: confirmText !== t("settings.confirmWord") }}
        onOk={onReset}
        okText={t("common.ok")}
        cancelText={t("common.cancel")}
      >
        <Space direction="vertical" style={{ width: "100%" }}>
          <Typography.Paragraph>{t("settings.resetWarning")}</Typography.Paragraph>
          <Input
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            placeholder={t("settings.confirmWord")}
          />
        </Space>
      </Modal>
    </Space>
  );
}
