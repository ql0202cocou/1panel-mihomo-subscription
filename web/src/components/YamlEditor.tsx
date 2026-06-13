import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { yaml } from "@codemirror/lang-yaml";
import { forwardRef } from "react";

interface Props {
  value: string;
  onChange?: (value: string) => void;
  height?: string;
  readOnly?: boolean;
}

/// YAML-highlighted CodeMirror editor. Forwards a ref so callers can drive the
/// editor (e.g. jump to a validation-error line).
const YamlEditor = forwardRef<ReactCodeMirrorRef, Props>(function YamlEditor(
  { value, onChange, height, readOnly },
  ref,
) {
  return (
    <CodeMirror
      ref={ref}
      value={value}
      height={height ?? "240px"}
      extensions={[yaml()]}
      editable={!readOnly}
      basicSetup={{ lineNumbers: true, foldGutter: false }}
      onChange={onChange}
    />
  );
});

export default YamlEditor;
