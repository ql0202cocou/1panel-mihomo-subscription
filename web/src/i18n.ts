import i18n from "i18next";
import { initReactI18next } from "react-i18next";

// Chinese is the only MVP locale, but all copy goes through keys from day one
// so English (and the app package's other locales) can be added without a
// refactor.
const zh = {
  translation: {
    app: { title: "Mihomo 订阅管理" },
    nav: { profiles: "订阅配置", settings: "系统设置", logout: "退出登录" },
    login: {
      title: "管理员登录",
      username: "管理员账户",
      password: "管理员密码",
      submit: "登录",
      failed: "账户或密码错误",
      tooMany: "尝试次数过多，请稍后再试",
    },
    profiles: {
      title: "订阅配置",
      create: "新建配置",
      name: "配置名称",
      sourceType: "原始订阅类型",
      sourceUrl: "机场订阅 URL",
      enabled: "已启用",
      disabled: "已禁用",
      lastFetch: "最近拉取",
      empty: "还没有订阅配置，点击“新建配置”开始。",
      open: "管理",
    },
    detail: {
      hostedLink: "托管订阅链接",
      copy: "复制链接",
      copied: "已复制",
      qrcode: "二维码",
      back: "返回列表",
      editingHint: "配置编辑功能将在下一步提供。",
    },
    settings: {
      title: "系统设置",
      publicPathPrefix: "公共路径前缀",
      resetPublicPath: "重置公共路径",
      resetWarning:
        "重置后，所有配置的托管链接将立即失效，所有客户端需要重新导入。输入 RESET 确认。",
      confirmWord: "RESET",
    },
    common: { cancel: "取消", ok: "确定", save: "保存", create: "创建" },
  },
};

void i18n.use(initReactI18next).init({
  resources: { zh },
  lng: "zh",
  fallbackLng: "zh",
  interpolation: { escapeValue: false },
});

export default i18n;
