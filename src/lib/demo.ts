/**
 * Sample data for `npm run dev:web` with `?demo`, so the interface can be
 * seen and screenshotted in a browser without the native side.
 */

import type { Job } from "./model";
import { newJob } from "./model";
import type { Entry, HistoryItem, Listing } from "./native";

const e = (path: string, size: number | null, isDir = false): Entry => ({ path, size, isDir, encrypted: false, modified: null });

function listing(label: string, archiveSize: number, entries: Entry[], extra: Partial<Listing> = {}): Listing {
  return {
    format: "zip",
    formatLabel: label,
    stem: "",
    entries,
    totalSize: entries.reduce((s, x) => s + (x.size ?? 0), 0),
    archiveSize,
    encrypted: false,
    pieces: 1,
    unsafeNames: 0,
    ...extra,
  };
}

let cached: Job[] | null = null;

export function demoJobs(): Job[] {
  if (cached) return cached;
  const home = "/Users/you/Downloads";
  const photos = newJob(`${home}/Iceland Trip 2026.zip`);
  photos.listing = listing("ZIP", 2_410_000_000, [e("Iceland Trip 2026", null, true), e("Iceland Trip 2026/IMG_0001.HEIC", 3_200_000)]);
  photos.destination = home;
  photos.state = { phase: "extracting", done: 1_520_000_000, total: 2_630_000_000, current: "Iceland Trip 2026/IMG_0412.HEIC" };

  const project = newJob(`${home}/design-assets.7z`);
  project.listing = listing(
    "7Z",
    184_000_000,
    [
      e("design-assets", null, true),
      e("design-assets/brand", null, true),
      e("design-assets/brand/logo.svg", 18_400),
      e("design-assets/brand/logo@2x.png", 412_000),
      e("design-assets/brand/palette.ase", 3_100),
      e("design-assets/fonts", null, true),
      e("design-assets/fonts/Inter-Variable.ttf", 803_000),
      e("design-assets/mockups", null, true),
      e("design-assets/mockups/home.fig", 96_400_000),
      e("design-assets/mockups/onboarding.fig", 71_200_000),
      e("design-assets/README.md", 2_400),
    ],
    { formatLabel: "7Z" },
  );
  project.destination = home;
  project.state = { phase: "browsing" };

  const film = newJob(`${home}/Season 2.part1.rar`);
  film.listing = listing("RAR", 8_900_000_000, [e("Season 2/Episode 1.mkv", 1_480_000_000)], { pieces: 6, encrypted: true });
  film.state = { phase: "waiting" };

  const docs = newJob(`${home}/Contracts.zip`);
  docs.listing = listing("ZIP", 4_800_000, [e("Contracts/a.pdf", 5_100_000)]);
  docs.state = { phase: "done", outcome: { output: `${home}/Contracts`, files: 23, bytes: 5_100_000, skipped: 4, unsafeNames: 0 } };

  const broken = newJob(`${home}/backup-2019.tar.gz`);
  broken.listing = listing("TAR.GZ", 1_200_000_000, []);
  broken.state = { phase: "failed", error: { kind: "damaged", detail: "It ends early, which usually means the download did not finish." } };

  cached = [photos, project, film, docs, broken];
  return cached;
}

export function demoHistory(): HistoryItem[] {
  const now = Math.floor(Date.now() / 1000);
  const home = "/Users/you/Downloads";
  return [
    { archive: `${home}/Contracts.zip`, output: `${home}/Contracts`, at: now - 120, files: 23, bytes: 5_100_000, error: null },
    { archive: `${home}/backup-2019.tar.gz`, output: "", at: now - 900, files: 0, bytes: 0, error: "The archive is damaged or incomplete." },
    { archive: `${home}/fonts.zip`, output: `${home}/fonts`, at: now - 7200, files: 214, bytes: 48_000_000, error: null },
    { archive: `${home}/invoice-march.pdf.gz`, output: `${home}/invoice-march.pdf`, at: now - 86400 * 2, files: 1, bytes: 240_000, error: null },
  ];
}
