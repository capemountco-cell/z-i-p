import { Gear, History, Logo, Plus, Tray } from "./Icons";

export type View = "unpack" | "history" | "settings";

interface Props {
  view: View;
  active: number;
  onView: (view: View) => void;
  onOpen: () => void;
}

/**
 * The window's top, laid out like Camu's (a classic Mac toolbar): the title
 * row holds the window buttons and the wordmark, centred; the row below
 * holds large icon-and-label buttons, places on the left and actions on the
 * right. Empty space in either row drags the window.
 */
export function TopBar({ view, active, onView, onOpen }: Props) {
  const place = (id: View, label: string, keys: string, icon: React.ReactNode, badge?: number) => (
    <button
      type="button"
      className={`tool${view === id ? " current" : ""}`}
      aria-current={view === id ? "page" : undefined}
      title={`${label} (${keys})`}
      onClick={() => onView(id)}
    >
      {icon}
      <span className="tool-label">{label}</span>
      {badge ? <span className="tool-badge">{badge}</span> : null}
    </button>
  );

  return (
    <header className="toolbar-shell" data-tauri-drag-region>
      <div className="titlerow" data-tauri-drag-region>
        <span className="title-logo" data-tauri-drag-region>
          <Logo size={18} />
          <span className="wordmark">Z-I-P</span>
        </span>
      </div>
      <nav className="toolrow" aria-label="Main" data-tauri-drag-region>
        <div className="toolgroup" data-tauri-drag-region>
          {place("unpack", "Unpack", "⌘1", <Tray className="i tool-icon" />, active)}
          {place("history", "History", "⌘2", <History className="i tool-icon" />)}
        </div>
        <div className="toolgroup" data-tauri-drag-region>
          <button type="button" className="tool narrow" onClick={onOpen} title="Open archives (⌘O)">
            <Plus className="i tool-icon" />
            <span className="tool-label">Open</span>
          </button>
          {place("settings", "Settings", "⌘,", <Gear className="i tool-icon" />)}
        </div>
      </nav>
    </header>
  );
}
