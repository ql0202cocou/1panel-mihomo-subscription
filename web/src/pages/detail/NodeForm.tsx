import { AutoComplete, Divider, Form, Input } from "antd";
import { useTranslation } from "react-i18next";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { commonFields, commonKeys, groupsFor, NODE_TYPES, type FieldDef } from "./nodeSchema";
import { AdvancedFields, FieldInput, getPath, isEmptyValue, setPath, splitAdvanced } from "./fields";

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
    if (k.trim() === "" || isEmptyValue(v)) continue;
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
    if (isEmptyValue(v)) delete fields[key];
    else fields[key] = v;
    onChange({ ...value, fields });
  }

  // Edit one subfield of a nested option block (e.g. `reality-opts.public-key`).
  // The group object is pruned and removed entirely once it has no entries left.
  function setGroupField(groupKey: string, path: string, v: unknown) {
    const current = value.fields[groupKey];
    const nested = current && typeof current === "object" && !Array.isArray(current)
      ? (current as Record<string, unknown>)
      : {};
    const next = setPath(nested, path, v);
    const fields = { ...value.fields };
    if (Object.keys(next).length === 0) delete fields[groupKey];
    else fields[groupKey] = next;
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

      {commonFields(value.type)
        .filter((def) => !def.showWhen || def.showWhen(value.fields))
        .map((def) => (
          <Form.Item key={def.key} label={fieldLabel(def)}>
            <FieldInput
              def={def}
              value={value.fields[def.key]}
              onChange={(v) => setField(def.key, v)}
            />
          </Form.Item>
        ))}

      {groupsFor(value.type)
        .filter((group) => !group.showWhen || group.showWhen(value.fields))
        .map((group) => {
          const nested = (value.fields[group.key] ?? {}) as Record<string, unknown>;
          return (
            <div key={group.key}>
              <Divider orientation="left" plain>
                {t(`nodeGroups.${group.key}`, group.key)}
              </Divider>
              {group.fields.map((sub) => (
                <Form.Item key={sub.key} label={fieldLabel(sub)}>
                  <FieldInput
                    def={sub}
                    value={getPath(nested, sub.key)}
                    onChange={(v) => setGroupField(group.key, sub.key, v)}
                  />
                </Form.Item>
              ))}
            </div>
          );
        })}

      <AdvancedFields entries={advanced} onChange={setAdvanced} />
    </Form>
  );
}
