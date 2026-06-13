import { useState } from "react";
import { useLocation, useNavigate, type Location } from "react-router-dom";
import { Alert, Button, Card, Form, Input } from "antd";
import { useTranslation } from "react-i18next";
import { useAuth } from "../auth";
import type { ApiError } from "../api";

export default function Login() {
  const { t } = useTranslation();
  const { login } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const from = (location.state as { from?: Location } | null)?.from?.pathname ?? "/";

  async function onFinish(values: { username: string; password: string }) {
    setError(null);
    setSubmitting(true);
    try {
      await login(values.username, values.password);
      navigate(from, { replace: true });
    } catch (e) {
      const err = e as ApiError;
      setError(err.status === 429 ? t("login.tooMany") : t("login.failed"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div style={{ display: "grid", placeItems: "center", minHeight: "100vh" }}>
      <Card title={t("login.title")} style={{ width: 360 }}>
        {error && (
          <Alert type="error" message={error} style={{ marginBottom: 16 }} showIcon />
        )}
        <Form layout="vertical" onFinish={onFinish}>
          <Form.Item
            name="username"
            label={t("login.username")}
            rules={[{ required: true }]}
          >
            <Input autoComplete="username" />
          </Form.Item>
          <Form.Item
            name="password"
            label={t("login.password")}
            rules={[{ required: true }]}
          >
            <Input.Password autoComplete="current-password" />
          </Form.Item>
          <Button type="primary" htmlType="submit" block loading={submitting}>
            {t("login.submit")}
          </Button>
        </Form>
      </Card>
    </div>
  );
}
