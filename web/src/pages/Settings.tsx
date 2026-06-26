import { useEffect, useState } from "react";
import { Button, Input, Modal, Segmented, message } from "antd";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import type { Settings as SettingsData } from "../types";
import { useTheme, type ThemeMode } from "../theme";
import "./Settings.css";

export default function Settings() {
  const { t } = useTranslation();
  const { mode, setMode } = useTheme();
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
    <div className="page-settings">
      <div className="settings-help">{t("settings.help")}</div>

      <div className="settings-card">
        <div className="settings-card-title">{t("settings.publicPathPrefix")}</div>
        <div className="settings-card-desc">{t("settings.publicPathDesc")}</div>
        <div className="settings-row">
          <span className="code-box">{settings?.public_path_prefix}</span>
          <Button danger onClick={() => setConfirming(true)}>
            {t("settings.resetPublicPath")}
          </Button>
        </div>
      </div>

      <div className="settings-card">
        <div className="settings-card-title">{t("settings.appearance")}</div>
        <div className="settings-row">
          <span className="settings-row-label">{t("settings.themeLabel")}</span>
          <Segmented
            value={mode}
            onChange={(v) => setMode(v as ThemeMode)}
            options={[
              { label: t("theme.light"), value: "light" },
              { label: t("theme.dark"), value: "dark" },
            ]}
          />
        </div>
      </div>

      <Modal
        title={t("settings.resetPublicPath")}
        open={confirming}
        onCancel={() => setConfirming(false)}
        okButtonProps={{ danger: true, disabled: confirmText !== t("settings.confirmWord") }}
        onOk={onReset}
        okText={t("common.ok")}
        cancelText={t("common.cancel")}
      >
        <p style={{ marginTop: 0 }}>{t("settings.resetWarning")}</p>
        <Input
          value={confirmText}
          onChange={(e) => setConfirmText(e.target.value)}
          placeholder={t("settings.confirmWord")}
        />
      </Modal>
    </div>
  );
}
