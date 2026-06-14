// Shared structured-editor widgets used by both the node editor and the group
// options editor: a typed input driven by a `FieldDef`, and an "advanced" block
// of free-form key/value rows (scalars get typed inputs; nested objects edit as
// a small YAML block) so admins never have to hand-write raw config.

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
import { useTranslation } from "react-i18next";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import type { FieldDef } from "./nodeSchema";

/** A typed input for one known field. The label is the caller's responsibility. */
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

/** Editable list of advanced key/value rows, kept in document order. */
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

/** Value editor for an advanced row, typed by the current value's JS type. */
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

/** Nested object/array advanced value, edited as a small YAML block. */
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
          // Keep the last valid parse while the YAML is mid-edit.
        }
      }}
    />
  );
}

/** Split an object into known (by key) and ordered advanced [key, value] pairs. */
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

/** Read a possibly dotted path (e.g. `headers.Host`) from a nested object. */
export function getPath(obj: Record<string, unknown>, path: string): unknown {
  return path.split(".").reduce<unknown>((acc, k) => (isObject(acc) ? acc[k] : undefined), obj);
}

/**
 * Immutably set/clear a dotted path in a nested object, pruning any parent
 * objects left empty. Returns the new root (empty when nothing remains).
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
