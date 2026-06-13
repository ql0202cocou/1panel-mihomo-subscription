import { Layout, Menu, Button, Space } from "antd";
import { Link, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useAuth } from "../auth";

const { Header, Content } = Layout;

export default function AppLayout() {
  const { t } = useTranslation();
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();

  // Highlight the top-level section.
  const selected = location.pathname.startsWith("/settings") ? "/settings" : "/";

  async function onLogout() {
    await logout();
    navigate("/login", { replace: true });
  }

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Header style={{ display: "flex", alignItems: "center" }}>
        <div style={{ color: "#fff", fontWeight: 600, marginRight: 32 }}>
          {t("app.title")}
        </div>
        <Menu
          theme="dark"
          mode="horizontal"
          selectedKeys={[selected]}
          style={{ flex: 1, minWidth: 0 }}
          items={[
            { key: "/", label: <Link to="/">{t("nav.profiles")}</Link> },
            {
              key: "/settings",
              label: <Link to="/settings">{t("nav.settings")}</Link>,
            },
          ]}
        />
        <Space style={{ color: "#fff" }}>
          <span>{user}</span>
          <Button onClick={onLogout}>{t("nav.logout")}</Button>
        </Space>
      </Header>
      <Content style={{ padding: 24, maxWidth: 1080, width: "100%", margin: "0 auto" }}>
        <Outlet />
      </Content>
    </Layout>
  );
}
