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

type BatchItem = { path: string; scan: WorkbookScan | null; error?: string };

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
  // 单文件详情模式
  const [path, setPath] = useState<string | null>(null);
  const [scan, setScan] = useState<WorkbookScan | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<string | null>(null);

  // 多文件批量模式
  const [batch, setBatch] = useState<BatchItem[]>([]);

  const [policy, setPolicy] = useState<CrossRefPolicy>("keep");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<SplitOutcome[]>([]);

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

  const batchRefs = useMemo(
    () =>
      batch.reduce(
        (n, b) =>
          n + (b.scan?.sheets.reduce((m, s) => m + s.cross_refs.length, 0) ?? 0),
        0
      ),
    [batch]
  );

  async function pickFile() {
    setError(null);
    setResults([]);
    setBatch([]);
    try {
      const sel = await open({
        multiple: true,
        filters: [{ name: "Excel 工作簿", extensions: ["xlsx", "xlsm"] }],
      });
      if (!sel) return;

      const list = Array.isArray(sel) ? sel : [sel];

      if (list.length === 1) {
        setPath(list[0]);
        setBusy(true);
        setScan(null);
        const s = await api.excelScan(list[0]);
        setScan(s);
        setPicked(new Set(s.sheets.filter((x) => x.state === "visible").map((x) => x.name)));
        setBusy(false);
        return;
      }

      // 批量模式：逐个扫描，单个失败不影响其他
      setPath(null);
      setScan(null);
      setBusy(true);
      const items: BatchItem[] = [];
      for (const p of list) {
        try {
          items.push({ path: p, scan: await api.excelScan(p) });
        } catch (e) {
          items.push({ path: p, scan: null, error: errText(e) });
        }
      }
      setBatch(items);
    } catch (e) {
      setError(errText(e));
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
      setResults(await api.excelSplit(path, [...picked], null, policy));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function doSplitBatch() {
    const usable = batch.filter((b) => b.scan);
    if (usable.length === 0) return;
    setBusy(true);
    setError(null);
    setResults([]);

    const all: SplitOutcome[] = [];
    const failures: string[] = [];

    for (const item of usable) {
      const sheets = (item.scan?.sheets ?? [])
        .filter((s) => s.state === "visible")
        .map((s) => s.name);
      if (sheets.length === 0) continue;
      try {
        const out = await api.excelSplit(item.path, sheets, null, policy);
        all.push(...out);
      } catch (e) {
        failures.push(`${baseName(item.path)}：${errText(e)}`);
      }
    }

    setResults(all);
    if (failures.length > 0) setError(failures.join("\n"));
    setBusy(false);
  }

  function refTargets(s: SheetInfo) {
    const targets = new Set<string>();
    s.cross_refs.forEach((r) => r.targets.forEach((t) => targets.add(t)));
    return [...targets];
  }

  // ── 空态 ─────────────────────────────
  if (!scan && batch.length === 0) {
    return (
      <>
        <div className="dropzone" onClick={pickFile}>
          <div className="dz-icon">
            <i />
            <i />
            <i />
          </div>
          <b>选择一个或多个 Excel 工作簿</b>
          <span>按工作表拆成独立文件，保留样式与打印设置</span>
          <span className="dz-hint">支持 .xlsx / .xlsm · 可多选批量处理</span>
        </div>
        {error && <div className="error-box">{error}</div>}
        {busy && <div className="muted center">正在读取工作簿…</div>}
      </>
    );
  }

  // ── 批量模式 ─────────────────────────
  if (batch.length > 0) {
    const totalSheets = batch.reduce(
      (n, b) => n + (b.scan?.sheets.filter((s) => s.state === "visible").length ?? 0),
      0
    );

    return (
      <>
        <div className="card">
          <div className="card-head">
            <div className="file-title">已选择 {batch.length} 个工作簿</div>
            <div className="card-head-right">
              <span className="muted small">共 {totalSheets} 个可见工作表</span>
              <button className="ghost" onClick={pickFile}>
                重选
              </button>
            </div>
          </div>
          <div className="result-list">
            {batch.map((b) => {
              const visible = b.scan?.sheets.filter((s) => s.state === "visible") ?? [];
              const hidden = b.scan?.sheets.filter((s) => s.state !== "visible") ?? [];
              const refs =
                b.scan?.sheets.reduce((m, s) => m + s.cross_refs.length, 0) ?? 0;
              return (
                <div key={b.path} className="result-row">
                  <span className={`st-dot ${b.error ? "err" : "ok"}`} />
                  <span className="result-name" title={b.path}>
                    {baseName(b.path)}
                  </span>
                  {b.error ? (
                    <span className="muted small">{b.error}</span>
                  ) : (
                    <span className="muted small">
                      {visible.length} 个可见表
                      {hidden.length > 0 ? ` · ${hidden.length} 个隐藏表不拆` : ""}
                      {refs > 0 ? ` · 跨表引用 ${refs}` : ""}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {batchRefs > 0 && (
          <div className="warn-box">
            <b>其中有 {batchRefs} 处跨表引用</b>
            <p>拆分后这些公式无法再访问其他工作表，请选择处理方式：</p>
            <PolicyPicker policy={policy} setPolicy={setPolicy} />
          </div>
        )}

        <div className="actions-bar">
          <span className="muted small">每个文件输出到各自所在目录</span>
          <span className="grow" />
          <button
            className="primary"
            disabled={busy || totalSheets === 0}
            onClick={doSplitBatch}
          >
            {busy ? "处理中…" : `拆分全部 ${totalSheets} 个工作表`}
          </button>
        </div>

        {error && <div className="error-box" style={{ whiteSpace: "pre-line" }}>{error}</div>}
        <ResultList results={results} />
      </>
    );
  }

  // ── 单文件详情模式 ────────────────────
  return (
    <>
      <div className="card">
        <div className="card-head">
          <div className="file-title" title={path ?? ""}>
            {path ? baseName(path) : ""}
          </div>
          <div className="card-head-right">
            <span className="muted">{scan!.sheet_count} 个工作表</span>
            <button className="ghost" onClick={pickFile}>
              换一个
            </button>
          </div>
        </div>

        <div className="sheet-list">
          {scan!.sheets.map((s) => {
            const targets = refTargets(s);
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
                  <button className="link" onClick={() => setDetail(isOpen ? null : s.name)}>
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

      {scan!.defined_name_refs.length > 0 && (
        <div className="warn-box">
          <b>定义名称中存在 {scan!.defined_name_refs.length} 处跨表引用</b>
          <p>拆分后这些名称可能失效，Excel 可能提示链接错误。</p>
          <div className="ref-detail">
            {scan!.defined_name_refs.slice(0, 6).map((d) => (
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
          <PolicyPicker policy={policy} setPolicy={setPolicy} />
        </div>
      )}

      <div className="actions-bar">
        <span className="muted small">输出到源文件所在目录</span>
        <span className="grow" />
        <button className="primary" disabled={picked.size === 0 || busy} onClick={doSplit}>
          {busy ? "处理中…" : `拆分 ${picked.size} 个工作表`}
        </button>
      </div>

      {error && <div className="error-box">{error}</div>}
      <ResultList results={results} />
    </>
  );
}

function PolicyPicker({
  policy,
  setPolicy,
}: {
  policy: CrossRefPolicy;
  setPolicy: (p: CrossRefPolicy) => void;
}) {
  return (
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
  );
}

function ResultList({ results }: { results: SplitOutcome[] }) {
  if (results.length === 0) return null;
  return (
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
  );
}
