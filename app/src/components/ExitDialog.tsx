interface Props {
  taskCount: number;
  onCancel: () => void;
  onMinimize: () => void;
  onExit: () => void;
}

/**
 * 退出确认（PRD F6-7）。
 *
 * 设计要点：摩擦只出现在退出，不出现在最小化。
 * 有任务时把「最小化并继续」作为首选按钮，用默认选项引导用户往安全方向走。
 */
export default function ExitDialog({ taskCount, onCancel, onMinimize, onExit }: Props) {
  const busy = taskCount > 0;

  return (
    <div className="overlay">
      <div className="dialog">
        {busy ? (
          <>
            <h3>还有 {taskCount} 个任务正在进行</h3>
            <p>最小化到托盘可以继续在后台完成，退出会中断这些任务。</p>
            <div className="actions">
              <button className="ghost" onClick={onCancel}>
                取消
              </button>
              <button onClick={onExit}>仍然退出</button>
              <button className="primary" onClick={onMinimize}>
                最小化并继续
              </button>
            </div>
          </>
        ) : (
          <>
            <h3>确定要退出青丘湖硅基工坊吗？</h3>
            <p>退出后将不再驻留托盘，需要重新启动才能继续使用。</p>
            <div className="actions">
              <button className="ghost" onClick={onCancel}>
                取消
              </button>
              <button className="primary" onClick={onExit}>
                退出
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
