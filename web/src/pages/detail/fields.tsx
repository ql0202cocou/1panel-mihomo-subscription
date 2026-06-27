// 节点编辑器和代理组选项编辑器共用的结构化编辑控件:由 `FieldDef` 驱动的带类型
// 输入框,以及一个「高级」自由键值行区块(标量给类型化输入框;嵌套对象用一小段
// YAML 编辑),让管理员永远不必手写原始配置。

import { useState } from "react";
import {
  AutoComplete,
  Button,
  Divider,
  Input,
  InputNumber,
  Select,
  Space,
  Switch,
  Typography,
} from "antd";
import { DownOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import type { FieldDef } from "./nodeSchema";
import "./modal.css";

/** 可点选的类型 chip 行(节点/分组类型选择)。自由输入仍由调用方旁边的控件处理;chip 是快捷选择。 */
export function TypeChips({
  options,
  value,
  onChange,
}: {
  options: readonly string[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="type-chips">
      {options.map((o) => (
        <span
          key={o}
          className={`type-chip${value === o ? " active" : ""}`}
          onClick={() => onChange(o)}
        >
          {o}
        </span>
      ))}
    </div>
  );
}

/** 单个已知字段的带类型输入框。标签由调用方负责。 */
export function FieldInput({
  def,
  value,
  onChange,
}: {
  def: FieldDef;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  switch (def.kind) {
    case "number":
      return (
        <InputNumber
          style={{ width: "100%" }}
          value={typeof value === "number" ? value : undefined}
          onChange={(n) => onChange(n)}
          placeholder={def.placeholder}
        />
      );
    case "switch":
      return <Switch checked={value === true} onChange={(c) => onChange(c)} />;
    case "password":
      return (
        <Input.Password
          value={value == null ? "" : String(value)}
          onChange={(e) => onChange(e.target.value)}
        />
      );
    case "select":
      return (
        <AutoComplete
          style={{ width: "100%" }}
          suffixIcon={<DownOutlined />}
          options={(def.options ?? []).map((o) => ({ value: o }))}
          value={value == null ? "" : String(value)}
          onChange={(s) => onChange(s)}
          placeholder={def.placeholder}
          allowClear
          filterOption={(input, opt) =>
            (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      );
    case "tags":
      return (
        <Select
          mode="tags"
          style={{ width: "100%" }}
          value={Array.isArray(value) ? (value as string[]) : []}
          onChange={(v) => onChange(v)}
          options={(def.options ?? []).map((o) => ({ value: o }))}
          tokenSeparators={[","]}
          placeholder={def.placeholder}
        />
      );
    default:
      return (
        <Input
          value={value == null ? "" : String(value)}
          placeholder={def.placeholder}
          onChange={(e) => onChange(e.target.value)}
        />
      );
  }
}

/** 可编辑的高级键值行列表,保持文档顺序。 */
export function AdvancedFields({
  entries,
  onChange,
}: {
  entries: [string, unknown][];
  onChange: (next: [string, unknown][]) => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <Divider orientation="left" plain>
        {t("fields.advanced")}
      </Divider>
      <Typography.Paragraph type="secondary" style={{ marginTop: -8 }}>
        {t("fields.advancedHint")}
      </Typography.Paragraph>
      {entries.map(([k, v], i) => (
        <Space key={`adv-${i}`} align="start" style={{ display: "flex", marginBottom: 8 }}>
          <Input
            style={{ width: 180 }}
            placeholder={t("fields.key")}
            value={k}
            onChange={(e) => {
              const next = entries.slice();
              next[i] = [e.target.value, v];
              onChange(next);
            }}
          />
          <AdvancedValue
            value={v}
            onChange={(nv) => {
              const next = entries.slice();
              next[i] = [k, nv];
              onChange(next);
            }}
          />
          <Button danger onClick={() => onChange(entries.filter((_, j) => j !== i))}>
            {t("fields.remove")}
          </Button>
        </Space>
      ))}
      <Button onClick={() => onChange([...entries, ["", ""]])} style={{ marginTop: 4 }}>
        {t("fields.addField")}
      </Button>
    </>
  );
}

/** 高级行的值编辑器,按当前值的 JS 类型决定输入形态。 */
function AdvancedValue({
  value,
  onChange,
}: {
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  if (typeof value === "boolean") {
    return <Switch checked={value} onChange={onChange} />;
  }
  if (typeof value === "number") {
    return <InputNumber style={{ width: 260 }} value={value} onChange={(n) => onChange(n)} />;
  }
  if (value !== null && typeof value === "object") {
    return <ObjectField value={value} onChange={onChange} />;
  }
  return (
    <Input
      style={{ width: 260 }}
      value={value == null ? "" : String(value)}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

/** 嵌套对象/数组的高级值,用一小段 YAML 编辑。 */
function ObjectField({
  value,
  onChange,
}: {
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const [text, setText] = useState(() => stringifyYaml(value).trimEnd());
  return (
    <Input.TextArea
      style={{ width: 260, fontFamily: "monospace" }}
      autoSize={{ minRows: 2, maxRows: 8 }}
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        try {
          onChange(parseYaml(e.target.value));
        } catch {
          // YAML 编辑中途解析失败时,保留上一次有效的解析结果。
        }
      }}
    />
  );
}

/** 把对象拆成已知字段(按 key)和有序的高级 [key, value] 对。 */
export function splitAdvanced(
  obj: Record<string, unknown>,
  known: Set<string>,
): [string, unknown][] {
  return Object.entries(obj).filter(([k]) => !known.has(k));
}

function isObject(v: unknown): v is Record<string, unknown> {
  return v != null && typeof v === "object" && !Array.isArray(v);
}

export function isEmptyValue(v: unknown): boolean {
  return (
    v === "" ||
    v === undefined ||
    v === null ||
    (Array.isArray(v) && v.length === 0) ||
    (isObject(v) && Object.keys(v).length === 0)
  );
}

/** 从嵌套对象里读取可能带点的路径(如 `headers.Host`)。 */
export function getPath(obj: Record<string, unknown>, path: string): unknown {
  return path.split(".").reduce<unknown>((acc, k) => (isObject(acc) ? acc[k] : undefined), obj);
}

/**
 * 在嵌套对象里以不可变方式设置/清除带点路径,并剪除因此变空的父对象。
 * 返回新的根对象(全部清空时为空对象)。
 */
export function setPath(
  obj: Record<string, unknown>,
  path: string,
  value: unknown,
): Record<string, unknown> {
  const keys = path.split(".");
  const root: Record<string, unknown> = { ...obj };
  if (keys.length === 1) {
    if (isEmptyValue(value)) delete root[keys[0]];
    else root[keys[0]] = value;
    return root;
  }
  const [head, ...rest] = keys;
  const child = isObject(root[head]) ? (root[head] as Record<string, unknown>) : {};
  const nextChild = setPath(child, rest.join("."), value);
  if (Object.keys(nextChild).length === 0) delete root[head];
  else root[head] = nextChild;
  return root;
}
