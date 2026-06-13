import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import App from "./App";
import { AuthProvider } from "./auth";
import "./i18n";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ConfigProvider>
      <BrowserRouter>
        <AuthProvider>
          <App />
        </AuthProvider>
      </BrowserRouter>
    </ConfigProvider>
  </StrictMode>,
);
