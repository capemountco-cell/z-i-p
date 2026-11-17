import type { Settings } from "../lib/native";
import { tildify } from "../lib/model";

interface Props {
  settings: Settings;
  version: string;
  onChange: (next: Settings) => void;
  onChooseFolder: () => void;
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button type="button" role="switch" aria-checked={on} aria-label={label} className={`switch${on ? " on" : ""}`} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  );
}

function Row({ title, detail, children }: { title: string; detail: string; children: React.ReactNode }) {
  return (
    <div className="setting">
      <div className="setting-text">
        <span className="setting-title">{title}</span>
        <span className="setting-detail">{detail}</span>
      </div>
      {children}
    </div>
  );
}

/** One page of settings, in plain words, each with what it does. */
export function SettingsView({ settings, version, onChange, onChooseFolder }: Props) {
  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => onChange({ ...settings, [key]: value });

  return (
    <div className="settings">
      <section className="settings-group">
        <h2>Opening an archive</h2>
        <Row title="When you open an archive" detail="Unpack it straight away, or look inside first to pick the files you want.">
          <select className="select" value={settings.onOpen} onChange={(e) => set("onOpen", e.target.value as Settings["onOpen"])}>
            <option value="extract">Unpack it</option>
            <option value="browse">Show what’s inside</option>
          </select>
        </Row>
      </section>

      <section className="settings-group">
        <h2>Where files go</h2>
        <Row
          title="Put unpacked files"
          detail={
            settings.destination === "folder"
              ? settings.folder
                ? `In ${tildify(settings.folder)}`
                : "Choose a folder."
              : settings.destination === "ask"
                ? "You choose a folder each time."
                : "In the same folder as the archive. If that folder can’t be written to, Z-I-P asks."
          }
        >
          <div className="setting-controls">
            {settings.destination === "folder" && (
              <button type="button" className="btn" onClick={onChooseFolder}>
                Choose…
              </button>
            )}
            <select
              className="select"
              value={settings.destination}
              onChange={(e) => {
                const destination = e.target.value as Settings["destination"];
                onChange({ ...settings, destination });
                if (destination === "folder" && !settings.folder) onChooseFolder();
              }}
            >
              <option value="nextToArchive">Next to the archive</option>
              <option value="ask">Ask every time</option>
              <option value="folder">In a folder I choose</option>
            </select>
          </div>
        </Row>
        <Row title="Avoid folders inside folders" detail="An archive holding one folder unpacks to that folder, not to “Photos/Photos”.">
          <Toggle on={settings.smartFolder} onChange={(v) => set("smartFolder", v)} label="Avoid folders inside folders" />
        </Row>
        <Row title="Show in Finder when done" detail="Opens a Finder window with the unpacked files selected.">
          <Toggle on={settings.revealWhenDone} onChange={(v) => set("revealWhenDone", v)} label="Show in Finder when done" />
        </Row>
      </section>

      <section className="settings-group">
        <h2>Cleanliness and safety</h2>
        <Row title="Leave out clutter" detail="Skips __MACOSX, .DS_Store, ._ files and Windows’ Thumbs.db.">
          <Toggle on={settings.skipJunk} onChange={(v) => set("skipJunk", v)} label="Leave out clutter" />
        </Row>
        <Row
          title="Check downloaded apps"
          detail="Files from a downloaded archive keep the “downloaded from the internet” mark, so macOS checks apps inside it as it would have checked the archive."
        >
          <Toggle on={settings.keepQuarantine} onChange={(v) => set("keepQuarantine", v)} label="Check downloaded apps" />
        </Row>
      </section>

      <section className="settings-group">
        <h2>How Z-I-P keeps your files safe</h2>
        <ul className="promises">
          <li>Your archive is never changed or deleted.</li>
          <li>Nothing is overwritten. If “Photos” exists, you get “Photos 2”.</li>
          <li>Files appear only when every one is out. A failed or stopped unpack leaves nothing behind.</li>
          <li>Free space is checked before the first byte is written.</li>
          <li>Files whose names point outside the folder are refused.</li>
          <li>Passwords stay in memory for this session and are never saved. Z-I-P makes no network connections.</li>
        </ul>
      </section>

      <p className="about faint">Z-I-P {version}</p>
    </div>
  );
}
