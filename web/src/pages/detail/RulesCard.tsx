import { useEffect, useRef, useState } from "react";
import { Button, Card, Space, Typography, message } from "antd";
import { useTranslation } from "react-i18next";
import { EditorSelection } from "@codemirror/state";
import type { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { api, type ApiError } from "../../api";
import YamlEditor from "../../components/YamlEditor";

interface Props {
  profileId: string;
  initial: string;
  /// Validation errors from the last generate attempt (itemized).
  errors: string[];
  onSaved: () => void;
}

/// Parse a "rules line N ..." message to its 1-based line number.
function lineOf(message: string): number | null {
  const m = message.match(/rules line (\d+)/);
  return m ? Number(m[1]) : null;
}

export default function RulesCard({ profileId, initial, errors, onSaved }: Props) {
  const { t } = useTranslation();
  const [content, setContent] = useState(initial);
  const editorRef = useRef<ReactCodeMirrorRef>(null);

  useEffect(() => {
    setContent(initial);
  }, [initial]);

  async function save() {
    try {
      await api(`/api/profiles/${profileId}/rules`, {
        method: "PUT",
        body: JSON.stringify({ content }),
      });
      message.success(t("rules.saved"));
      onSaved();
    } catch (e) {
      message.error((e as ApiError).message ?? "保存失败");
    }
  }

  function jumpTo(line: number) {
    const view = editorRef.current?.view;
    if (!view) return;
    const lineInfo = view.state.doc.line(Math.min(line, view.state.doc.lines));
    view.dispatch({
      selection: EditorSelection.cursor(lineInfo.from),
      scrollIntoView: true,
    });
    view.focus();
  }

  return (
    <Card
      title={t("rules.title")}
      extra={
        <Button type="primary" onClick={save}>
          {t("rules.save")}
        </Button>
      }
    >
      <Space direction="vertical" style={{ width: "100%" }} size="small">
        <Typography.Text type="secondary">{t("rules.hint")}</Typography.Text>
        <YamlEditor ref={editorRef} value={content} onChange={setContent} height="260px" />
        {errors.length > 0 && (
          <div>
            {errors.map((err, i) => {
              const line = lineOf(err);
              return (
                <div key={i} style={{ color: "#cf1322", marginTop: 4 }}>
                  {err}
                  {line && (
                    <Button type="link" size="small" onClick={() => jumpTo(line)}>
                      {t("rules.jump")}
                    </Button>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </Space>
    </Card>
  );
}
