import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText } from "../lib/api";
import type { CrossRefPolicy, SheetInfo, SplitOutcome, WorkbookScan } from "../lib/types";

function baseName(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** 仅开发环境：访问 ?demo=1 时用这份数据预览列表与告警的渲染效果。 */
const DEMO_SCAN: WorkbookScan = {
  path: "C:\\demo\\季度报表.xlsx",
  sheet_count: 3,
  sheets: [
    {
      name: "汇总",
      state: "visible",
      rows: 48,
      cols: 9,
      has_chart: true,
      cross_refs: [
        { cell: "B3", formula: "SUM(明细1!C2:C31)", targets: ["明细1"] },
        { cell: "C3", formula: "SUM('明细 2'!C2:C31)", targets: ["明细 2"] },
      ],
      truncated: false,
    },
    { name: "明细1", state: "visible", rows: 120, cols: 7, has_chart: false, cross_refs: [], truncated: false },
    { name: "明细 2", state: "hidden", rows: 96, cols: 7, has_chart: false, cross_refs: [], truncated: false },
  ],
  defined_name_refs: [],
};

export default function ExcelPanel() {
  const [path, setPath] = useState<string | null>(null);
  const [scan, setScan] = useState<WorkbookScan | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [policy, setPolicy] = useState<CrossRefPolicy>("keep");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<SplitOutcome[]>([]);
  const [detail, setDetail] = useState<string | null>(null);

  useEffect(() => {
    if (import.meta.env.DEV && new URLSearchParams(location.search).has("demo")) {
      setPath(DEMO_SCAN.path);
      setScan(DEMO_SCAN);
      setPicked(new Set(["汇总", "明细1"]));
    }
  }, []);

  const pickedWithRefs = useMemo(() => {
    if (!scan) return 0;
    return scan.sheets
      .filter((s) => picked.has(s.name))
      .reduce((n, s) => n + s.cross_refs.length, 0);
  }, [scan, picked]);

  async function pickFile() {
    setError(null);
    setResults([]);
    try {
      const sel = await open({
        multiple: false,
        filters: [{ name: "Excel 工作簿", extensions: ["xlsx", "xlsm"] }],
      });
      if (typeof sel !== "string") return;

      setPath(sel);
      setBusy(true);
      setScan(null);
      const s = await api.excelScan(sel);
      setScan(s);
      setPicked(new Set(s.sheets.filter((x) => x.state === "visible").map((x) => x.name)));
    } catch (e) {
      setError(errText(e));
      setScan(null);
    } finally {
      setBusy(false);
    }
  }

  function toggle(name: string) {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }

  async function doSplit() {
    if (!path || picked.size === 0) return;
    setBusy(true);
    setError(null);
    setResults([]);
    try {
      const out = await api.excelSplit(path, [...picked], null, policy);
      setResults(out);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  function refSummary(s: SheetInfo) {
    const targets = new Set<string>();
    s.cross_refs.forEach((r) => r.targets.forEach((t) => targets.add(t)));
    return [...targets];
  }

  if (!scan) {
    return (
      <>
        <div className="dropzone" onClick={pickFile}>
          <div className="dz-icon">
            <i />
            <i />
            <i />
          </div>
          <b>选择一个 Excel 工作簿</b>
          <span>按工作表拆成多个独立文件，保留样式与打印设置</span>
          <span className="dz-hint">支持 .xlsx / .xlsm</span>
        </div>
        {error && <div className="error-box">{error}</div>}
        {busy && <div className="muted center">正在读取工作簿…</div>}
      </>
    );
  }

  return (
    <>
      <div className="card">
        <div className="card-head">
          <div className="file-title" title={path ?? ""}>
            {path ? baseName(path) : ""}
          </div>
          <div className="card-head-right">
            <span className="muted">{scan.sheet_count} 个工作表</span>
            <button className="ghost" onClick={pickFile}>
              换一个
            </button>
          </div>
        </div>

        <div className="sheet-list">
          {scan.sheets.map((s) => {
            const targets = refSummary(s);
            const isOpen = detail === s.name;
            return (
              <div key={s.name} className={`sheet-row ${picked.has(s.name) ? "on" : ""}`}>
                <label className="sheet-main">
                  <input
                    type="checkbox"
                    checked={picked.has(s.name)}
                    onChange={() => toggle(s.name)}
                  />
                  <span className="sheet-name">{s.name}</span>
                  {s.state !== "visible" && <span className="badge">隐藏</span>}
                  {s.has_chart && <span className="badge">含图表</span>}
                  {s.cross_refs.length > 0 && (
                    <span className="badge warn">跨表引用 {s.cross_refs.length}</span>
                  )}
                  <span className="muted small">
                    {s.rows} 行 × {s.cols} 列
                  </span>
                </label>
                {s.cross_refs.length > 0 && (
                  <button
                    className="link"
                    onClick={() => setDetail(isOpen ? null : s.name)}
                  >
                    {isOpen ? "收起" : `查看 → ${targets.join("、")}`}
                  </button>
                )}
                {isOpen && (
                  <div className="ref-detail">
                    {s.cross_refs.slice(0, 12).map((r) => (
                      <div key={r.cell} className="ref-line">
                        <code>{r.cell}</code>
                        <span>= {r.formula}</span>
                      </div>
                    ))}
                    {s.cross_refs.length > 12 && (
                      <div className="muted small">
                        还有 {s.cross_refs.length - 12} 处未显示
                        {s.truncated ? "（已达显示上限）" : ""}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {scan.defined_name_refs.length > 0 && (
        <div className="warn-box">
          <b>定义名称中存在 {scan.defined_name_refs.length} 处跨表引用</b>
          <p>拆分后这些名称可能失效，Excel 可能提示链接错误。</p>
          <div className="ref-detail">
            {scan.defined_name_refs.slice(0, 6).map((d) => (
              <div key={d.cell} className="ref-line">
                <code>{d.cell}</code>
                <span> → {d.targets.join("、")}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {pickedWithRefs > 0 && (
        <div className="warn-box">
          <b>选中的工作表含 {pickedWithRefs} 处跨表引用</b>
          <p>拆分后这些公式无法再访问其他工作表，请选择处理方式：</p>
          <div className="radio-row">
            <label>
              <input
                type="radio"
                name="policy"
                checked={policy === "keep"}
                onChange={() => setPolicy("keep")}
              />
              保留公式<span className="muted">（可能出现 #REF!）</span>
            </label>
            <label>
              <input
                type="radio"
                name="policy"
                checked={policy === "to_value"}
                onChange={() => setPolicy("to_value")}
              />
              转为当前值<span className="muted">（需文件曾被 Office 保存过）</span>
            </label>
            <label>
              <input
                type="radio"
                name="policy"
                checked={policy === "abort"}
                onChange={() => setPolicy("abort")}
              />
              中止拆分
            </label>
          </div>
        </div>
      )}

      <div className="actions-bar">
        <span className="muted small">输出到源文件所在目录</span>
        <span className="grow" />
        <button
          className="primary"
          disabled={picked.size === 0 || busy}
          onClick={doSplit}
        >
          {busy ? "处理中…" : `拆分 ${picked.size} 个工作表`}
        </button>
      </div>

      {error && <div className="error-box">{error}</div>}

      {results.length > 0 && (
        <div className="card">
          <div className="card-head">
            <div className="file-title">已生成 {results.length} 个文件</div>
          </div>
          <div className="result-list">
            {results.map((r) => (
              <div key={r.path} className="result-row">
                <span className="ok-dot" />
                <span className="result-name" title={r.path}>
                  {baseName(r.path)}
                </span>
                <span className="muted small">{fmtSize(r.bytes)}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
