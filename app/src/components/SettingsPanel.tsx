import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errText } from "../lib/api";
import type { AppConfig, Feature, UpdateChannel } from "../lib/types";
import type { UpdateInfo } from "../lib/types";

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export default function SettingsPanel({ features }: { features: Feature[] }) {
  const [closeToTray, setCloseToTray] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [version, setVersion] = useState("");
  const [error, setError] = useState<string | null>(null);

  const [cfg, setCfg] = useState<AppConfig | null>(null);
  const [endpointDraft, setEndpointDraft] = useState("");
  const [endpointSaved, setEndpointSaved] = useState(false);

  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateErr, setUpdateErr] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);

  useEffect(() => {
    api
      .getCloseBehavior()
      .then(setCloseToTray)
      .catch((e) => {
        // 纯浏览器调试环境没有后端，不展示这类原始错误
        if (!import.meta.env.DEV) setError(errText(e));
      });
    api
      .getAutostart()
      .then(setAutostart)
      .catch(() => undefined);
    api
      .appVersion()
      .then(setVersion)
      .catch(() => undefined);
    api
      .getAppConfig()
      .then((c) => {
        setCfg(c);
        setEndpointDraft(c.updateEndpoint ?? "");
      })
      .catch(() => undefined);
  }, []);

  // 下载进度由后端推送，避免前端轮询
  useEffect(() => {
    const p = listen<{ downloaded: number; total: number | null }>(
      "forge://update-progress",
      (e) => {
        const { downloaded, total } = e.payload;
        setProgress(
          total ? `${fmtBytes(downloaded)} / ${fmtBytes(total)}` : fmtBytes(downloaded)
        );
      }
    );
    return () => {
      p.then((un) => un()).catch(() => undefined);
    };
  }, []);

  async function change(toTray: boolean) {
    setCloseToTray(toTray);
    try {
      await api.setCloseBehavior(toTray);
    } catch (e) {
      if (!import.meta.env.DEV) setError(errText(e));
    }
  }

  async function changeAutostart(enable: boolean) {
    setAutostart(enable);
    try {
      await api.setAutostart(enable);
    } catch (e) {
      setAutostart(!enable);
      setError(errText(e));
    }
  }

  async function changeChannel(channel: UpdateChannel) {
    if (!cfg) return;
    setCfg({ ...cfg, updateChannel: channel });
    try {
      await api.setUpdateChannel(channel);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function saveEndpoint() {
    try {
      await api.setUpdateEndpoint(endpointDraft);
      setEndpointSaved(true);
      setUpdateErr(null);
      setUpdate(null);
      window.setTimeout(() => setEndpointSaved(false), 2000);
    } catch (e) {
      // 地址格式错误必须让用户看到，不能静默吞掉
      setError(errText(e));
    }
  }

  async function changeCheckOnStart(enable: boolean) {
    if (!cfg) return;
    setCfg({ ...cfg, checkUpdateOnStart: enable });
    try {
      await api.setCheckOnStart(enable);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function doCheck() {
    setChecking(true);
    setUpdateErr(null);
    setUpdate(null);
    try {
      setUpdate(await api.checkUpdate());
    } catch (e) {
      setUpdateErr(errText(e));
    } finally {
      setChecking(false);
    }
  }

  async function doInstall() {
    setInstalling(true);
    setProgress(null);
    try {
      // Windows 上安装程序启动后本进程会退出，之后的代码不会执行
      await api.installUpdate();
    } catch (e) {
      setUpdateErr(errText(e));
      setInstalling(false);
    }
  }

  const delivered = features.filter((f) => f.delivered).length;
  const pending = features.filter((f) => !f.delivered && f.status !== "out");

  return (
    <>
      <div className="card">
        <div className="card-head">
          <div className="file-title">窗口行为</div>
        </div>
        <div className="opt-row">
          <span className="opt-label">关闭按钮</span>
          <div className="chips">
            <button
              className={`chip ${closeToTray ? "on" : ""}`}
              onClick={() => change(true)}
            >
              最小化到托盘
            </button>
            <button
              className={`chip ${!closeToTray ? "on" : ""}`}
              onClick={() => change(false)}
            >
              直接退出
            </button>
          </div>
        </div>
        <div className="opt-row">
          <span className="opt-label" />
          <span className="muted small">
            无论选哪项，最小化都不会中断正在执行的任务；退出时一定会弹确认提示。
          </span>
        </div>
        <div className="opt-row">
          <span className="opt-label">开机自启</span>
          <label className="check">
            <input
              type="checkbox"
              checked={autostart}
              onChange={(e) => changeAutostart(e.target.checked)}
            />
            登录 Windows 后自动启动并驻留托盘
          </label>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="file-title">更新</div>
          <span className="muted small">当前 v{version || "—"}</span>
        </div>

        <div className="opt-row">
          <span className="opt-label">通道</span>
          <div className="chips">
            <button
              className={`chip ${cfg?.updateChannel !== "beta" ? "on" : ""}`}
              onClick={() => changeChannel("stable")}
            >
              稳定版
            </button>
            <button
              className={`chip ${cfg?.updateChannel === "beta" ? "on" : ""}`}
              onClick={() => changeChannel("beta")}
            >
              测试版
            </button>
          </div>
        </div>

        <div className="opt-row">
          <span className="opt-label">更新源</span>
          <input
            className="input wide"
            placeholder="留空则使用内置地址（随通道变化）"
            value={endpointDraft}
            onChange={(e) => setEndpointDraft(e.target.value)}
            spellCheck={false}
          />
          <button className="chip" onClick={saveEndpoint}>
            {endpointSaved ? "已保存" : "保存"}
          </button>
        </div>

        <div className="opt-row">
          <span className="opt-label">启动检查</span>
          <label className="check">
            <input
              type="checkbox"
              checked={cfg?.checkUpdateOnStart ?? true}
              onChange={(e) => changeCheckOnStart(e.target.checked)}
            />
            启动时自动检查（发现新版本仅提示，不自动安装）
          </label>
        </div>

        <div className="opt-row">
          <span className="opt-label" />
          <button className="btn" onClick={doCheck} disabled={checking || installing}>
            {checking ? "检查中…" : "检查更新"}
          </button>
          {update?.available && (
            <button className="btn primary" onClick={doInstall} disabled={installing}>
              {installing ? "下载中…" : `安装 v${update.version}`}
            </button>
          )}
          {progress && <span className="muted small">已下载 {progress}</span>}
        </div>

        {update && !update.available && (
          <div className="result-row">
            <span className="st-dot ok" />
            <span className="result-name">已是最新版本</span>
            <span className="muted small">{update.endpoint}</span>
          </div>
        )}

        {update?.available && (
          <div className="result-row">
            <span className="st-dot wait" />
            <span className="result-name">发现新版本 v{update.version}</span>
            <span className="muted small">{update.date ?? ""}</span>
          </div>
        )}

        {update?.available && update.notes && (
          <div className="muted small" style={{ padding: "4px 2px 8px", whiteSpace: "pre-wrap" }}>
            {update.notes}
          </div>
        )}

        {updateErr && <div className="error-box">{updateErr}</div>}
      </div>

      <div className="card">
        <div className="card-head">
          <div className="file-title">功能状态</div>
          <span className="muted small">
            已交付 {delivered} 项 · 待交付 {pending.length} 项
          </span>
        </div>
        <div className="result-list">
          {pending.slice(0, 8).map((f) => (
            <div key={f.id} className="result-row">
              <span className="st-dot wait" />
              <span className="result-name">{f.name}</span>
              <span className="muted small">
                {f.status === "deferred"
                  ? "已延后"
                  : f.milestone
                    ? `计划 ${f.milestone}`
                    : "待排期"}
              </span>
            </div>
          ))}
          {pending.length === 0 && (
            <div className="result-row">
              <span className="muted small">全部功能均已交付</span>
            </div>
          )}
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div className="file-title">关于</div>
        </div>
        <div className="opt-row">
          <span className="opt-label">版本</span>
          <span className="small">v{version || "—"}</span>
        </div>
        <div className="opt-row">
          <span className="opt-label">配置格式</span>
          <span className="small muted">
            schemaVersion {cfg?.schemaVersion ?? "—"}（升级时自动迁移并备份）
          </span>
        </div>
        <div className="opt-row">
          <span className="opt-label">引擎</span>
          <span className="small muted">未检测（文档转换模块 M3 提供）</span>
        </div>
        <div className="opt-row">
          <span className="opt-label">数据</span>
          <span className="small muted">全部处理在本机完成，不上传任何文件</span>
        </div>
      </div>

      {error && <div className="error-box">{error}</div>}
    </>
  );
}
