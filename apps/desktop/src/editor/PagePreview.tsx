import { EditorContent, useEditor } from "@tiptap/react";

import { editorExtensions } from "./extensions";
import { tryParseMarkdownToJSON } from "./markdown";

export interface PagePreviewProps {
  draftBody: string;
  parseError: string | null;
}

export function PagePreview({ draftBody, parseError }: PagePreviewProps) {
  const parsed = parseError ? null : tryParseMarkdownToJSON(draftBody);
  const canRender = parsed?.ok === true;
  const content = canRender ? parsed.json : { type: "doc", content: [{ type: "paragraph" }] };

  const editor = useEditor({
    extensions: editorExtensions,
    content,
    editable: false,
    editorProps: {
      attributes: {
        "aria-readonly": "true",
      },
    },
  });

  if (!canRender) {
    const message = parseError ?? parsed?.error ?? "Could not render preview";
    return (
      <div className="page-preview-fallback">
        <p className="error-text">{message}</p>
        <pre className="page-preview-raw">{draftBody}</pre>
      </div>
    );
  }

  return <EditorContent editor={editor} className="markdown-body page-editor-content page-preview" />;
}
