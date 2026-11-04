import { useEffect, useState } from "react";
import { api, errText } from "../lib/api";
import type { Feature } from "../lib/types";

export default function SettingsPanel({ features }: { features: Feature[] }) {
  const [closeToTray, setCloseToTray] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [version, setVersion] = useState("0.2.0");
  const [error, setError] = useState<string | null>(null);

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
          <div className="file-title">功能状态</div>
          <span className="muted small">
            已交付 {delivered} 项 · 待交付 {pending.length} 项
          </span>
        </div>
        <div className="result-list">
          {pending.slice(0, 8).map((f) => (
            <div key={f.id} className="result-row">
              <span className={`st-dot ${f.status === "conditional" ? "wait" : "wait"}`} />
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
          <span className="small">v{version}</span>
        </div>
        <div className="opt-row">
          <span className="opt-label">引擎</span>
          <span className="small muted">未检测（文档转换模块 M3 提供）</span>
        </div>
        <div className="opt-row">
          <span className="opt-label">自动更新</span>
          <span className="small muted">内网更新通道将在 M4 提供</span>
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
