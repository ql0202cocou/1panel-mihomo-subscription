import { Navigate, useLocation } from "react-router-dom";
import { Spin } from "antd";
import type { ReactNode } from "react";
import { useAuth } from "../auth";

/// 守卫受保护路由;未登录时跳转 /login 并保留原本要访问的路径。
export default function RequireAuth({ children }: { children: ReactNode }) {
  const { user, loading } = useAuth();
  const location = useLocation();

  if (loading) {
    return (
      <div style={{ display: "grid", placeItems: "center", minHeight: "100vh" }}>
        <Spin size="large" />
      </div>
    );
  }
  if (!user) {
    return <Navigate to="/login" state={{ from: location }} replace />;
  }
  return <>{children}</>;
}
