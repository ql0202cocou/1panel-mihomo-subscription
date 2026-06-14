import { AutoComplete, Form, Input } from "antd";
import { useTranslation } from "react-i18next";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { commonFields, commonKeys, NODE_TYPES, type FieldDef } from "./nodeSchema";
import { AdvancedFields, FieldInput, splitAdvanced } from "./fields";

export interface NodeModel {
  name: string;
  type: string;
  /** Every proxy key except `name`/`type`, in document order. */
  fields: Record<string, unknown>;
}

/** Parse stored proxy YAML into the editor model; tolerant of invalid input. */
export function contentToModel(content: string): NodeModel {
  let parsed: Record<string, unknown> = {};
  try {
    const p = parseYaml(content);
    if (p && typeof p === "object" && !Array.isArray(p)) {
      parsed = p as Record<string, unknown>;
    }
  } catch {
    // Unparseable content edits as a blank node rather than blocking the admin.
  }
  let name = "";
  let type = "";
  const fields: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(parsed)) {
    if (k === "name") name = typeof v === "string" ? v : String(v ?? "");
    else if (k === "type") type = typeof v === "string" ? v : String(v ?? "");
    else fields[k] = v;
  }
  return { name, type, fields };
}

/** Serialize the editor model back to a Mihomo proxy YAML mapping. */
export function modelToContent(m: NodeModel): string {
  const obj: Record<string, unknown> = {};
  if (m.name) obj.name = m.name;
  if (m.type) obj.type = m.type;
  for (const [k, v] of Object.entries(m.fields)) {
    if (k.trim() === "" || v === "" || v === undefined || v === null) continue;
    obj[k] = v;
  }
  return stringifyYaml(obj);
}

interface Props {
  value: NodeModel;
  onChange: (next: NodeModel) => void;
}

/** Structured editor for a custom proxy node: typed common fields per type plus
 *  advanced key/value rows for everything else — no hand-written YAML required. */
export default function NodeForm({ value, onChange }: Props) {
  const { t } = useTranslation();
  const known = commonKeys(value.type);

  function setField(key: string, v: unknown) {
    const fields = { ...value.fields };
    if (v === "" || v === undefined || v === null) delete fields[key];
    else fields[key] = v;
    onChange({ ...value, fields });
  }

  // Advanced rows are kept as an ordered array so a key rename does not reorder
  // the surrounding fields. They are merged back into `fields` on every edit.
  const advanced = splitAdvanced(value.fields, known);

  function setAdvanced(next: [string, unknown][]) {
    const fields: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value.fields)) {
      if (known.has(k)) fields[k] = v;
    }
    for (const [k, v] of next) fields[k] = v;
    onChange({ ...value, fields });
  }

  function fieldLabel(def: FieldDef) {
    return t(`nodeFields.${def.key}`, def.key);
  }

  return (
    <Form layout="vertical">
      <Form.Item label={t("nodes.name")} required>
        <Input
          value={value.name}
          onChange={(e) => onChange({ ...value, name: e.target.value })}
        />
      </Form.Item>
      <Form.Item label={t("nodes.type")} required>
        <AutoComplete
          style={{ width: "100%" }}
          options={NODE_TYPES.map((o) => ({ value: o }))}
          value={value.type}
          onChange={(s) => onChange({ ...value, type: s })}
          placeholder="ss / vmess / vless / trojan / hysteria2"
          filterOption={(input, opt) =>
            (opt?.value ?? "").toLowerCase().includes(input.toLowerCase())
          }
        />
      </Form.Item>

      {commonFields(value.type).map((def) => (
        <Form.Item key={def.key} label={fieldLabel(def)}>
          <FieldInput
            def={def}
            value={value.fields[def.key]}
            onChange={(v) => setField(def.key, v)}
          />
        </Form.Item>
      ))}

      <AdvancedFields entries={advanced} onChange={setAdvanced} />
    </Form>
  );
}
