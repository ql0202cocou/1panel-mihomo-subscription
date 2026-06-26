import { useState, type FormEvent } from "react";
import { useLocation, useNavigate, type Location } from "react-router-dom";
import { Button, Input } from "antd";
import { DeploymentUnitOutlined, ExclamationCircleFilled } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useAuth } from "../auth";
import type { ApiError } from "../api";
import "./Login.css";

export default function Login() {
  const { t } = useTranslation();
  const { login } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const from = (location.state as { from?: Location } | null)?.from?.pathname ?? "/";

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await login(username, password);
      navigate(from, { replace: true });
    } catch (err) {
      const e = err as ApiError;
      setError(e.status === 429 ? t("login.tooMany") : t("login.failed"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="login-screen">
      <div className="login-caption">SELF-HOSTED · MIHOMO / CLASH SUBSCRIPTION MANAGER</div>
      <form className="login-card" onSubmit={onSubmit}>
        <div className="login-head">
          <span className="login-logo">
            <DeploymentUnitOutlined />
          </span>
          <span className="login-title">{t("login.title")}</span>
          <span className="login-sub">Mihomo {t("app.subtitle")}</span>
        </div>

        {error && (
          <div className="login-error">
            <ExclamationCircleFilled />
            <span>{error}</span>
          </div>
        )}

        <div className="login-field">
          <label className="login-label">{t("login.username")}</label>
          <Input
            size="large"
            autoComplete="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
        </div>
        <div className="login-field">
          <label className="login-label">{t("login.password")}</label>
          <Input.Password
            size="large"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>

        <Button
          type="primary"
          htmlType="submit"
          block
          loading={submitting}
          className="login-submit"
        >
          {t("login.submit")}
        </Button>
      </form>
    </div>
  );
}
