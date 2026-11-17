import { useCallback, useEffect, useRef, useState } from "react";

import { ContentsPanel } from "./components/ContentsPanel";
import { HistoryView } from "./components/HistoryView";
import { Tray } from "./components/Icons";
import { PasswordSheet } from "./components/PasswordSheet";
import { SettingsView } from "./components/SettingsView";
import { TopBar, type View } from "./components/TopBar";
import { UnpackView } from "./components/UnpackView";
import { demoHistory, demoJobs } from "./lib/demo";
import type { Job, JobState } from "./lib/model";
import { afterPasswordError, errorText, fileName, formatBytes, isActive, newJob, parentDir, rememberPassword, tildify } from "./lib/model";
import * as native from "./lib/native";
import { DEFAULT_SETTINGS, type HistoryItem, type Settings } from "./lib/native";

interface Toast {
  id: number;
  text: string;
  tone: "ok" | "error";
}

const demo = !native.nativeAvailable() && typeof location !== "undefined" && new URLSearchParams(location.search).has("demo");

export function App() {
  const [view, setView] = useState<View>("unpack");
  const [jobs, setJobs] = useState<Job[]>(() => (demo ? demoJobs() : []));
  const [selected, setSelected] = useState<number | null>(() => (demo ? (demoJobs()[1]?.id ?? null) : null));
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [history, setHistory] = useState<HistoryItem[]>(() => (demo ? demoHistory() : []));
  const [version, setVersion] = useState("");
  const [dragging, setDragging] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  // Bumped when an extraction ends, so the queue looks for the next one.
  const [tick, setTick] = useState(0);

  // The latest values, for async code that outlives a render.
  const jobsRef = useRef(jobs);
  jobsRef.current = jobs;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const passwords = useRef<string[]>([]);
  const running = useRef(false);
  const lastOutput = useRef<string | null>(null);

  const toast = useCallback((text: string, tone: Toast["tone"] = "ok") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-2), { id, text, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 5000);
  }, []);

  const update = useCallback((id: number, change: (job: Job) => Job) => {
    setJobs((all) => all.map((j) => (j.id === id ? change(j) : j)));
  }, []);
  const setState = useCallback((id: number, state: JobState) => update(id, (j) => ({ ...j, state })), [update]);

  /** Read what is inside, then queue or show it. */
  const read = useCallback(
    async (job: Job) => {
      try {
        const listing = await native.inspect(job.path, job.password);
        const browse = settingsRef.current.onOpen === "browse";
        update(job.id, (j) => ({ ...j, listing, state: { phase: browse ? "browsing" : "waiting" } }));
        if (browse) setSelected((s) => s ?? job.id);
      } catch (e) {
        const error = native.asCoreError(e);
        if (error.kind === "passwordRequired" || error.kind === "wrongPassword") {
          const next = afterPasswordError(job, error, passwords.current);
          if (next.state.phase === "waiting") {
            // A remembered password to try: the listing needs it first.
            const retry: Job = { ...next, state: { phase: "reading" } };
            update(job.id, () => retry);
            void read(retry);
          } else {
            update(job.id, () => next);
          }
        } else {
          setState(job.id, { phase: "failed", error });
        }
      }
    },
    [update, setState],
  );

  const addPaths = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      const fresh = paths.filter((p) => !jobsRef.current.some((j) => j.path === p && isActive(j))).map(newJob);
      setJobs((all) => [...fresh, ...all]);
      setView("unpack");
      fresh.forEach((j) => void read(j));
    },
    [read],
  );

  /** Unpack one archive. Only one runs at a time, so disks are not thrashed. */
  const start = useCallback(
    async (job: Job) => {
      running.current = true;
      try {
        const s = settingsRef.current;
        let destination = job.destination ?? (await native.destinationFor(job.path));
        destination ??= await native.chooseFolder(`Choose where to unpack “${job.name}”`, parentDir(job.path));
        if (!destination) {
          setState(job.id, { phase: "cancelled" });
          return;
        }
        update(job.id, (j) => ({ ...j, destination, state: { phase: "extracting", done: 0, total: 0, current: "" } }));
        const outcome = await native.extract(job.id, {
          archive: job.path,
          destination,
          password: job.password,
          only: job.only,
          skipJunk: s.skipJunk,
          smartFolder: s.smartFolder,
          keepQuarantine: s.keepQuarantine,
        });
        passwords.current = rememberPassword(passwords.current, job.password);
        setState(job.id, { phase: "done", outcome });
        lastOutput.current = outcome.output;
        toast(`${job.name} → ${fileName(outcome.output)}${outcome.skipped ? ` · ${outcome.skipped} clutter files left out` : ""}`);
      } catch (e) {
        const error = native.asCoreError(e);
        if (error.kind === "passwordRequired" || error.kind === "wrongPassword") {
          update(job.id, (j) => afterPasswordError(j, error, passwords.current));
        } else if (error.kind === "cancelled") {
          setState(job.id, { phase: "cancelled" });
        } else {
          setState(job.id, { phase: "failed", error });
          toast(`${job.name}: ${errorText(error)}`, "error");
        }
      } finally {
        running.current = false;
        setTick((t) => t + 1);
        native.getHistory().then(setHistory, () => {});
      }
    },
    [update, setState, toast],
  );

  // The queue: start the oldest waiting archive whenever nothing is running.
  // When the queue empties, show the last result in Finder (once per batch).
  useEffect(() => {
    if (demo || running.current || jobs.some((j) => j.state.phase === "extracting")) return;
    const next = [...jobs].reverse().find((j) => j.state.phase === "waiting");
    if (next) {
      void start(next);
    } else if (lastOutput.current && !jobs.some(isActive)) {
      if (settingsRef.current.revealWhenDone) void native.reveal(lastOutput.current).catch(() => {});
      lastOutput.current = null;
    }
  }, [jobs, start, tick]);

  // Progress from Rust.
  useEffect(() => {
    const off = native.onProgress((p) =>
      setJobs((all) =>
        all.map((j) => (j.id === p.job && j.state.phase === "extracting" ? { ...j, state: { phase: "extracting", done: p.done, total: p.total, current: p.current } } : j)),
      ),
    );
    return () => void off.then((f) => f());
  }, []);

  // Settings, history, version, and archives opened from Finder.
  useEffect(() => {
    if (!native.nativeAvailable()) return;
    native.getSettings().then(setSettings, () => {});
    native.getHistory().then(setHistory, () => {});
    native.appVersion().then(setVersion, () => {});
    const collect = () => native.takeOpened().then(addPaths, () => {});
    void collect();
    const off = native.onOpened(collect);
    return () => void off.then((f) => f());
  }, [addPaths]);

  // Drag and drop anywhere on the window.
  useEffect(() => {
    const off = native.onDragDrop((e) => {
      if (e.type === "over") setDragging(true);
      else {
        setDragging(false);
        if (e.type === "drop") addPaths(e.paths);
      }
    });
    return () => void off.then((f) => f());
  }, [addPaths]);

  const openArchives = useCallback(() => {
    void native.chooseArchives().then(addPaths);
  }, [addPaths]);

  const changeSettings = (next: Settings) => {
    setSettings(next);
    native.saveSettings(next).catch((e) => toast(`Couldn't save settings: ${native.asCoreError(e).kind}`, "error"));
  };

  const chooseSettingsFolder = async () => {
    const folder = await native.chooseFolder("Choose where unpacked files go", settings.folder ?? undefined);
    if (folder) changeSettings({ ...settings, destination: "folder", folder });
  };

  const prompt = jobs.find((j) => j.state.phase === "needsPassword") ?? null;

  const submitPassword = (password: string) => {
    if (!prompt) return;
    // The typed password goes to this archive, and is tried on the other
    // locked ones in the batch too.
    const changed = jobsRef.current
      .filter((j) => j.id === prompt.id || (j.state.phase === "needsPassword" && !j.passwordTyped))
      .map((j): Job => ({ ...j, password, passwordTyped: j.id === prompt.id, state: { phase: j.listing ? "waiting" : "reading" } }));
    setJobs((all) => all.map((j) => changed.find((c) => c.id === j.id) ?? j));
    changed.filter((j) => !j.listing).forEach((j) => void read(j));
  };

  const selectedJob = jobs.find((j) => j.id === selected) ?? null;

  // Keyboard: ⌘O open, ⌘1/⌘2/⌘, places, Esc closes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "o") {
        e.preventDefault();
        openArchives();
      } else if (e.metaKey && e.key === "1") setView("unpack");
      else if (e.metaKey && e.key === "2") setView("history");
      else if (e.metaKey && e.key === ",") {
        e.preventDefault();
        setView("settings");
      } else if (e.key === "Escape" && !prompt) setSelected(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openArchives, prompt]);

  const destinationHint =
    settings.destination === "nextToArchive"
      ? "Files go next to the archive."
      : settings.destination === "ask"
        ? "You choose where files go each time."
        : `Files go to ${settings.folder ? tildify(settings.folder) : "the folder chosen in Settings"}.`;

  const active = jobs.filter(isActive).length;
  const extracting = jobs.find((j) => j.state.phase === "extracting");
  const waiting = jobs.filter((j) => j.state.phase === "waiting").length;

  return (
    <div className="shell">
      <TopBar view={view} active={active} onView={setView} onOpen={openArchives} />
      <main className="main">
        <div className="content">
          {view === "unpack" && (
            <UnpackView
              jobs={jobs}
              selected={selected}
              destinationHint={destinationHint}
              onSelect={(id) => setSelected((s) => (s === id ? null : id))}
              onOpen={openArchives}
              onReveal={(j) => j.state.phase === "done" && void native.reveal(j.state.outcome.output).catch(() => {})}
              onCancel={(j) => (j.state.phase === "extracting" ? void native.cancel(j.id) : setState(j.id, { phase: "cancelled" }))}
              onRetry={(j) => {
                const next: Job = { ...j, state: { phase: j.listing ? "waiting" : "reading" } };
                update(j.id, () => next);
                if (!j.listing) void read(next);
              }}
              onRemove={(j) => {
                setJobs((all) => all.filter((x) => x.id !== j.id));
                setSelected((s) => (s === j.id ? null : s));
              }}
              onClearFinished={() => setJobs((all) => all.filter(isActive))}
            />
          )}
          {view === "history" && (
            <HistoryView
              items={history}
              onReveal={(p) => void native.reveal(p).catch(() => toast("That folder is no longer there.", "error"))}
              onClear={() => {
                setHistory([]);
                void native.clearHistory().catch(() => {});
              }}
            />
          )}
          {view === "settings" && <SettingsView settings={settings} version={version} onChange={changeSettings} onChooseFolder={chooseSettingsFolder} />}
          {view === "unpack" && selectedJob && (
            <ContentsPanel
              key={selectedJob.id}
              job={selectedJob}
              onClose={() => setSelected(null)}
              onExtract={(j, only) => update(j.id, (x) => ({ ...x, only, state: { phase: "waiting" } }))}
              onReveal={(j) => j.state.phase === "done" && void native.reveal(j.state.outcome.output).catch(() => {})}
              onChooseDestination={async (j) => {
                const folder = await native.chooseFolder(`Choose where to unpack “${j.name}”`, j.destination ?? parentDir(j.path));
                if (folder) update(j.id, (x) => ({ ...x, destination: folder }));
              }}
            />
          )}
        </div>
        <footer className="statusbar">
          <span>
            {extracting && extracting.state.phase === "extracting"
              ? `Unpacking ${extracting.name}${extracting.state.total ? ` · ${formatBytes(extracting.state.done)} of ${formatBytes(extracting.state.total)}` : ""}${waiting ? ` · ${waiting} waiting` : ""}`
              : active
                ? "Reading…"
                : "Ready"}
          </span>
          <span className="spacer" />
          <span className="faint">{destinationHint}</span>
        </footer>
      </main>

      {dragging && (
        <div className="drop-overlay">
          <div className="drop-target">
            <Tray className="dropzone-icon" />
            <span>Drop to unpack</span>
          </div>
        </div>
      )}

      {prompt && (
        <PasswordSheet
          job={prompt}
          waiting={jobs.filter((j) => j.id !== prompt.id && j.state.phase === "needsPassword" && !j.passwordTyped).length}
          onSubmit={submitPassword}
          onCancel={() => setState(prompt.id, { phase: "cancelled" })}
        />
      )}

      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast t-${t.tone}`}>
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}
