import { useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText } from "../lib/api";
import type { ConvertOptions } from "../lib/types";

const FORMATS = [
  { key: "jpg", label: "JPG" },
  { key: "png", label: "PNG" },
  { key: "webp", label: "WebP" },
  { key: "bmp", label: "BMP" },
  { key: "ico", label: "ICO" },
] as const;

type ItemState = {
  status: "wait" | "run" | "ok" | "err";
  msg?: string;
  out?: string;
};

function baseName(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

export default function ImagePanel() {
  const [files, setFiles] = useState<string[]>([]);
  const [format, setFormat] = useState<string>("jpg");
  const [quality, setQuality] = useState(85);
  const [maxEdge, setMaxEdge] = useState<string>("");
  const [background, setBackground] = useState("#FFFFFF");
  const [items, setItems] = useState<Record<string, ItemState>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const doneCount = useMemo(
    () => Object.values(items).filter((i) => i.status === "ok").length,
    [items]
  );

  async function pickFiles() {
    setError(null);
    try {
      const sel = await open({
        multiple: true,
        filters: [
          {
            name: "图片",
            extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "ico", "svg"],
          },
        ],
      });
      if (!sel) return;
      const list = Array.isArray(sel) ? sel : [sel];
      setFiles(list);
      setItems(Object.fromEntries(list.map((f) => [f, { status: "wait" as const }])));
    } catch (e) {
      setError(errText(e));
    }
  }

  async function run() {
    if (files.length === 0) return;
    setBusy(true);
    setError(null);

    const edge = maxEdge.trim() === "" ? null : Number(maxEdge);
    const options: ConvertOptions = {
      target_format: format,
      quality,
      max_edge: edge && edge > 0 ? edge : null,
      scale_percent: null,
      background: format === "jpg" || format === "bmp" || format === "ico" ? background : null,
    };

    for (const f of files) {
      setItems((prev) => ({ ...prev, [f]: { status: "run" } }));
      try {
        const out = await api.imageConvert(f, null, options);
        setItems((prev) => ({
          ...prev,
          [f]: { status: "ok", out: out.output, msg: out.note ?? undefined },
        }));
      } catch (e) {
        setItems((prev) => ({ ...prev, [f]: { status: "err", msg: errText(e) } }));
      }
    }

    setBusy(false);
  }

  return (
    <>
      <div className="dropzone compact" onClick={pickFiles}>
        <div className="dz-icon">
          <i />
          <i />
          <i />
        </div>
        <b>{files.length > 0 ? `已选择 ${files.length} 张图片` : "选择图片"}</b>
        <span>支持 PNG / JPG / WebP / BMP / GIF / TIFF / ICO / SVG</span>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="file-title">输出设置</div>
        </div>

        <div className="opt-row">
          <span className="opt-label">目标格式</span>
          <div className="chips">
            {FORMATS.map((f) => (
              <button
                key={f.key}
                className={`chip ${format === f.key ? "on" : ""}`}
                onClick={() => setFormat(f.key)}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>

        {format === "jpg" && (
          <div className="opt-row">
            <span className="opt-label">质量</span>
            <input
              type="range"
              min={1}
              max={100}
              value={quality}
              onChange={(e) => setQuality(Number(e.target.value))}
            />
            <span className="muted small num">{quality}</span>
          </div>
        )}

        {format === "webp" && (
          <div className="opt-row">
            <span className="opt-label" />
            <span className="muted small">WebP 当前为无损编码，质量参数不生效</span>
          </div>
        )}

        <div className="opt-row">
          <span className="opt-label">长边限制</span>
          <input
            className="input num"
            type="number"
            min={1}
            placeholder="不限"
            value={maxEdge}
            onChange={(e) => setMaxEdge(e.target.value)}
          />
          <span className="muted small">像素，留空则保持原尺寸</span>
        </div>

        {(format === "jpg" || format === "bmp" || format === "ico") && (
          <div className="opt-row">
            <span className="opt-label">透明铺底</span>
            <input
              type="color"
              value={background}
              onChange={(e) => setBackground(e.target.value)}
              className="color-input"
            />
            <span className="muted small">PNG 转 {format.toUpperCase()} 时使用</span>
          </div>
        )}

        <div className="opt-row">
          <span className="opt-label">元数据</span>
          <span className="muted small">
            输出文件不含 EXIF / GPS 等元数据（转码时自然丢弃）
          </span>
        </div>
      </div>

      {files.length > 0 && (
        <div className="card">
          <div className="card-head">
            <div className="file-title">转换列表</div>
            <span className="muted">{doneCount} / {files.length} 完成</span>
          </div>
          <div className="result-list">
            {files.map((f) => {
              const st = items[f] ?? { status: "wait" as const };
              return (
                <div key={f} className="result-row">
                  <span className={`st-dot ${st.status}`} />
                  <span className="result-name" title={f}>
                    {baseName(f)}
                  </span>
                  <span className="muted small">
                    {st.status === "run" && "处理中…"}
                    {st.status === "ok" && (st.msg ?? "完成")}
                    {st.status === "err" && st.msg}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      <div className="actions-bar">
        <span className="muted small">输出到源文件所在目录</span>
        <span className="grow" />
        <button className="primary" disabled={files.length === 0 || busy} onClick={run}>
          {busy ? "转换中…" : `转换 ${files.length} 张`}
        </button>
      </div>

      {error && <div className="error-box">{error}</div>}
    </>
  );
}
