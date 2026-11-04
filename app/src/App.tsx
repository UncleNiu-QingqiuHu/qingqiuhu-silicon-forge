import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errText } from "./lib/api";
import type { Feature, UpdateInfo } from "./lib/types";
import Placeholder from "./components/Placeholder";
import ExitDialog from "./components/ExitDialog";
import ExcelPanel from "./components/ExcelPanel";
import ImagePanel from "./components/ImagePanel";
import MarkdownPanel from "./components/MarkdownPanel";
import SettingsPanel from "./components/SettingsPanel";

const MODULES = [
  { key: "convert", label: "文档转换", hint: "Office → PDF", entry: "F1-1" },
  { key: "excel", label: "Excel 拆表", hint: "工作表拆分", entry: "F2-2" },
  { key: "markdown", label: "文档", hint: "Markdown", entry: "F3-1" },
  { key: "image", label: "图片", hint: "格式转换", entry: "F4-2" },
] as const;

type ModuleKey = (typeof MODULES)[number]["key"];
type ViewKey = ModuleKey | "settings";

/**
 * 仅开发环境使用的降级数据：在纯浏览器里跑 vite dev 时没有 Tauri 后端，
 * 用这份数据保证界面仍可调试。生产构建中该常量会被 tree-shake 移除。
 */
const DEV_FEATURES: Feature[] = [
  { id: "F1-1", name: "Word 转 PDF", module: "convert", status: "committed", milestone: "M3", placeholder: "文档转换模块将在 M3 阶段提供，需本机已安装 Office 或 WPS", blocked_by: [], delivered: false, available: true },
  { id: "F2-2", name: "按工作表拆分", module: "excel", status: "committed", milestone: "M1", placeholder: null, blocked_by: [], delivered: true, available: true },
  { id: "F3-1", name: "Markdown 编辑", module: "markdown", status: "committed", milestone: "M2", placeholder: null, blocked_by: [], delivered: true, available: true },
  { id: "F4-2", name: "图片格式转换", module: "image", status: "committed", milestone: "M1", placeholder: null, blocked_by: [], delivered: true, available: true },
  { id: "F7-1", name: "AI 助手对话", module: "ai", status: "conditional", milestone: "M5", placeholder: "AI 助手需要内网模型端点就绪后开放", blocked_by: ["PC-1", "PC-2", "PC-3"], delivered: false, available: false },
  { id: "F7-7", name: "PS 高级技能", module: "ai", status: "conditional", milestone: "M7", placeholder: "需要 Photoshop 2025 及以上版本", blocked_by: ["PC-1", "PC-2"], delivered: false, available: false },
];

function WorkArea({ view, feature, features }: { view: ViewKey; feature?: Feature; features: Feature[] }) {
  if (view === "settings") return <SettingsPanel features={features} />;
  if (!feature || !feature.available || !feature.delivered) {
    return <Placeholder feature={feature} />;
  }
  switch (view) {
    case "excel":
      return <ExcelPanel />;
    case "image":
      return <ImagePanel />;
    case "markdown":
      return <MarkdownPanel />;
    default:
      return <Placeholder feature={feature} />;
  }
}

function navBadge(f: Feature | undefined): { text: string; warn: boolean } | null {
  if (!f) return null;
  if (!f.available) return { text: "待排期", warn: true };
  if (!f.delivered) return { text: "开发中", warn: false };
  return null;
}

export default function App() {
  // 开发环境支持 ?view=settings 直达某个页面，便于截图与联调
  const [view, setView] = useState<ViewKey>(() => {
    if (import.meta.env.DEV) {
      const v = new URLSearchParams(location.search).get("view");
      if (v === "settings") return "settings";
      if (v && MODULES.some((m) => m.key === v)) return v as ModuleKey;
    }
    return "excel";
  });
  const [features, setFeatures] = useState<Feature[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [exitRequest, setExitRequest] = useState(false);
  const [fatal, setFatal] = useState<string | null>(null);
  const [version, setVersion] = useState("");
  const [configFatal, setConfigFatal] = useState<string | null>(null);
  const [updateBanner, setUpdateBanner] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    api
      .appVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);

  // 配置拒绝启动（schemaVersion 高于程序）时阻断；否则按设置做一次启动检查。
  useEffect(() => {
    (async () => {
      try {
        const blocked = await api.getConfigFatal();
        if (blocked) {
          setConfigFatal(blocked);
          return;
        }
      } catch {
        return; // 纯浏览器调试环境没有后端
      }
      try {
        const cfg = await api.getAppConfig();
        if (!cfg.checkUpdateOnStart) return;
        const info = await api.checkUpdate();
        if (info.available) setUpdateBanner(info);
      } catch {
        // 启动检查失败不打扰用户，设置页可手动检查
      }
    })();
  }, []);

  useEffect(() => {
    (async () => {
      try {
        setFeatures(await api.features());
      } catch (e) {
        if (import.meta.env.DEV) {
          setFeatures(DEV_FEATURES);
        } else {
          setFatal(errText(e));
        }
      } finally {
        setLoaded(true);
      }
    })();
  }, []);

  useEffect(() => {
    const subs: Array<Promise<() => void>> = [];

    subs.push(
      listen("forge://request-exit", () => setExitRequest(true)).catch(() => () => {})
    );
    subs.push(
      listen<string>("forge://nav", (e) => {
        if (e.payload === "settings" || e.payload === "progress") {
          setView(e.payload === "settings" ? "settings" : "excel");
        }
      }).catch(() => () => {})
    );

    return () => {
      subs.forEach((p) => p.then((un) => un()).catch(() => undefined));
    };
  }, []);

  const byId = (id: string) => features.find((f) => f.id === id);
  const isSettings = view === "settings";
  const current = MODULES.find((m) => m.key === view);
  const currentFeature = current ? byId(current.entry) : undefined;

  const handleExit = async () => {
    try {
      await api.confirmExit();
    } catch {
      window.close();
    }
  };

  const handleMinimize = async () => {
    setExitRequest(false);
    try {
      await api.hideToTray();
    } catch {
      /* 非 Tauri 环境忽略 */
    }
  };

  if (!loaded) return <div className="app" />;

  if (configFatal) {
    return (
      <div className="app">
        <main className="work" style={{ gridColumn: "1 / -1" }}>
          <div className="error-box">{configFatal}</div>
          <p className="muted small" style={{ marginTop: 12 }}>
            配置文件未被改动。请升级到新版本后再运行，或用旧版本备份的
            config.v*.bak 手动恢复。
          </p>
        </main>
      </div>
    );
  }

  if (fatal) {
    return (
      <div className="app">
        <main className="work" style={{ gridColumn: "1 / -1" }}>
          <div className="error-box">应用初始化失败：{fatal}</div>
        </main>
      </div>
    );
  }

  return (
    <div className="app">
      <nav className="nav">
        <div className="brand">
          <div className="brand-mark">
            <i />
            <i />
            <i />
          </div>
          <div className="brand-text">
            <b>硅基工坊</b>
            <span>Silicon Forge</span>
          </div>
        </div>

        {MODULES.map((m) => {
          const badge = navBadge(byId(m.entry));
          return (
            <button
              key={m.key}
              className={`nav-item ${view === m.key ? "active" : ""}`}
              onClick={() => setView(m.key)}
            >
              <span>
                <span className="label">{m.label}</span>
                <span className="hint">{m.hint}</span>
              </span>
              {badge && (
                <span className={`badge ${badge.warn ? "warn" : ""}`}>{badge.text}</span>
              )}
            </button>
          );
        })}

        <div className="nav-spacer" />

        <button
          className={`nav-item ${isSettings ? "active" : ""}`}
          onClick={() => setView("settings")}
        >
          <span>
            <span className="label">设置</span>
            <span className="hint">托盘 · 更新</span>
          </span>
        </button>
      </nav>

      <main className={`work ${view === "markdown" ? "flush" : ""}`}>
        {updateBanner && (
          <div className="update-banner">
            <span>
              发现新版本 <b>v{updateBanner.version}</b>
            </span>
            <span className="grow" />
            <button className="chip on" onClick={() => setView("settings")}>
              去更新
            </button>
            <button className="chip" onClick={() => setUpdateBanner(null)}>
              稍后
            </button>
          </div>
        )}
        {view !== "markdown" && (
          <div className="work-head">
            <h1>{isSettings ? "设置" : current?.label}</h1>
            <p>{isSettings ? "窗口行为与版本信息" : current?.hint}</p>
          </div>
        )}
        <WorkArea view={view} feature={currentFeature} features={features} />
      </main>

      <aside className={`ai ${aiOpen ? "" : "collapsed"}`}>
        <div className="ai-head">
          {aiOpen && <b>AI 助手</b>}
          <button
            className="ghost icon-btn"
            title={aiOpen ? "收起" : "展开"}
            onClick={() => setAiOpen((v) => !v)}
          >
            {aiOpen ? "»" : "«"}
          </button>
        </div>
        {aiOpen && (
          <div className="ai-body">
            <span className="badge warn">待排期 · M5</span>
            <p>
              AI 助手需要内网模型端点就绪后开放，
              届时可用自然语言编排复杂任务。
            </p>
          </div>
        )}
      </aside>

      <footer className="status">
        <span>
          <span className="dot off" />
          引擎未检测
        </span>
        <span>队列空闲</span>
        <span className="grow" />
        <span>v{version}</span>
      </footer>

      {exitRequest && (
        <ExitDialog
          taskCount={0}
          onCancel={() => setExitRequest(false)}
          onMinimize={handleMinimize}
          onExit={handleExit}
        />
      )}
    </div>
  );
}
