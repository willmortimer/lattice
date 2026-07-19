import { EditorContent, useEditor } from "@tiptap/react";

import { richEditorExtensions } from "./richEditorExtensions";
import { handleEditorLinkClick } from "./linkClick";
import { tryParseMarkdownToJSON } from "./markdown";

export interface PagePreviewProps {
  draftBody: string;
  parseError: string | null;
  /** Same workspace-link navigation as the live editor. */
  onOpenWiki?: (target: string) => void;
}

export function PagePreview({ draftBody, parseError, onOpenWiki }: PagePreviewProps) {
  const parsed = parseError ? null : tryParseMarkdownToJSON(draftBody);
  const canRender = parsed?.ok === true;
  const content = canRender ? parsed.json : { type: "doc", content: [{ type: "paragraph" }] };

  const editor = useEditor({
    extensions: richEditorExtensions,
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

  return (
    <div onClick={(event) => handleEditorLinkClick(event, onOpenWiki)}>
      <EditorContent editor={editor} className="markdown-body page-editor-content page-preview" />
    </div>
  );
}
