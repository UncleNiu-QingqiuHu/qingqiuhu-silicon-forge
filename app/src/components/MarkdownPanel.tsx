import { useCallback, useEffect, useRef, useState } from "react";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { markdown } from "@codemirror/lang-markdown";
import { basicSetup } from "codemirror";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, errText } from "../lib/api";
import type { ExportOutcome } from "../lib/types";

const SAMPLE = `# 示例文档

在左侧编辑 Markdown，右侧实时预览。

## 支持的内容

- **粗体**、*斜体*、~~删除线~~
- 行内代码 \`const a = 1\`
- [链接](https://www.qingqiuhu.com)

| 项目 | 数量 |
|---|---|
| 甲 | 3 |
| 乙 | 5 |

\`\`\`rust
fn main() {
    println!("你好");
}
\`\`\`

> 导出 PDF 时每页顶部会带上品牌页眉。
`;

/** 与应用配色一致的编辑器主题 */
const forgeTheme = EditorView.theme(
  {
    "&": {
      color: "#e5e9ed",
      backgroundColor: "#14171a",
      fontSize: "13px",
      height: "100%",
    },
    ".cm-scroller": {
      fontFamily:
        'ui-monospace, "Cascadia Mono", Consolas, "Microsoft YaHei", monospace',
      lineHeight: "1.7",
    },
    ".cm-content": { padding: "12px 0" },
    ".cm-gutters": {
      backgroundColor: "#14171a",
      color: "#4d565f",
      border: "none",
    },
    ".cm-activeLine": { backgroundColor: "#1b2026" },
    ".cm-activeLineGutter": { backgroundColor: "#1b2026", color: "#8b939d" },
    "&.cm-focused .cm-cursor": { borderLeftColor: "#14b8a6" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
      backgroundColor: "#1f3a38",
    },
    ".cm-selectionMatch": { backgroundColor: "#26333b" },
  },
  { dark: true }
);

function baseName(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export default function MarkdownPanel() {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const timerRef = useRef<number | null>(null);

  const [source, setSource] = useState(SAMPLE);
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ExportOutcome | null>(null);
  const [pageNumbers, setPageNumbers] = useState(true);

  const renderPreview = useCallback((md: string) => {
    if (timerRef.current) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(async () => {
      try {
        setPreview(await api.mdPreview(md));
      } catch {
        /* 预览失败不打断编辑 */
      }
    }, 180);
  }, []);

  useEffect(() => {
    if (!hostRef.current) return;

    const state = EditorState.create({
      doc: SAMPLE,
      extensions: [
        basicSetup,
        markdown(),
        forgeTheme,
        EditorView.lineWrapping,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) {
            const text = u.state.doc.toString();
            setSource(text);
            renderPreview(text);
          }
        }),
      ],
    });

    const view = new EditorView({ state, parent: hostRef.current });
    viewRef.current = view;
    renderPreview(SAMPLE);

    return () => {
      view.destroy();
      viewRef.current = null;
      if (timerRef.current) window.clearTimeout(timerRef.current);
    };
  }, [renderPreview]);

  function loadText(text: string, filePath: string | null) {
    setPath(filePath);
    setSource(text);
    setResult(null);
    setError(null);
    const view = viewRef.current;
    if (view) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
    }
    renderPreview(text);
  }

  async function openFile() {
    setError(null);
    try {
      const sel = await open({
        multiple: false,
        filters: [{ name: "Markdown", extensions: ["md", "markdown", "txt"] }],
      });
      if (typeof sel !== "string") return;
      const text = await api.readTextFile(sel);
      loadText(text, sel);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function saveFile() {
    setError(null);
    setBusy(true);
    try {
      let target = path;
      if (!target) {
        target = await save({
          filters: [{ name: "Markdown", extensions: ["md"] }],
          defaultPath: "未命名文档.md",
        });
        if (!target) {
          setBusy(false);
          return;
        }
      }
      await api.writeTextFile(target, source);
      setPath(target);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function doExport(format: "html" | "pdf") {
    setError(null);
    setBusy(true);
    setResult(null);
    try {
      const out = await api.mdExport(source, path, {
        format,
        title: path ? baseName(path).replace(/\.(md|markdown|txt)$/i, "") : "未命名文档",
        page_numbers: pageNumbers,
        out_dir: null,
      });
      setResult(out);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="md-toolbar">
        <button onClick={openFile}>打开</button>
        <button onClick={saveFile} disabled={busy}>
          保存
        </button>
        <span className="md-file" title={path ?? ""}>
          {path ? baseName(path) : "未命名文档"}
        </span>
        <span className="grow" />
        <label className="check small">
          <input
            type="checkbox"
            checked={pageNumbers}
            onChange={(e) => setPageNumbers(e.target.checked)}
          />
          页码
        </label>
        <button onClick={() => doExport("html")} disabled={busy}>
          导出 HTML
        </button>
        <button className="primary" onClick={() => doExport("pdf")} disabled={busy}>
          {busy ? "导出中…" : "导出 PDF"}
        </button>
      </div>

      <div className="md-split">
        <div className="md-pane">
          <div className="md-editor" ref={hostRef} />
        </div>
        <div className="md-pane preview">
          <div
            className="md-preview"
            // 内容由后端 comrak 渲染，已禁用 unsafe HTML
            dangerouslySetInnerHTML={{ __html: preview }}
          />
        </div>
      </div>

      {error && <div className="error-box">{error}</div>}

      {result && (
        <div className="result-bar">
          <span className="ok-dot" />
          <span className="result-name" title={result.path}>
            已导出 {baseName(result.path)}
          </span>
          <span className="muted small">{fmtSize(result.bytes)}</span>
        </div>
      )}
    </>
  );
}
