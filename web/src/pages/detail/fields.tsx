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
          filterOption={(input, opt) =>
            (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
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
