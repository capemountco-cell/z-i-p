# What Z-I-P must do better

Drawn from screenshots of competing products and their users' feedback
(BetterZip, Keka, The Unarchiver, MacZip, ReclaimArc, WinRAR, WinZip; Reddit
threads and App Store reviews), November 2026.

## What people complain about, and Z-I-P's answer

| Complaint | Z-I-P |
| --- | --- |
| Extraction fails half-way when the disk is nearly full, leaving "no usable output" | Free space checked first from the archive's own sizes; staging folder removed on any failure |
| No way to look inside and pick files (Keka: "huge flaw") | Contents panel with a box beside every file and folder, filter, "Unpack N selected" |
| First window "doesn't declare its intent"; two settings windows confuse | One window whose empty state says "Drop archives here to unpack them"; one Settings page in plain words |
| "Couldn't find my files" after a success notification | Every finished row has Show in Finder; Finder opens on the result after a batch; History lists where everything went |
| Unclear whether a password protects the archive | The password sheet names the archive and says it is protected; the contents panel shows "Encrypted with a password" |
| Other tools "lost files", "freeze on launch" | The archive is never touched; nothing is overwritten; one archive unpacks at a time on a background thread |
| "Access Denied" and other cryptic errors | Every failure mapped to a sentence that says what to do (damaged download, missing part, full disk, can't write) |
| Mac clutter (`__MACOSX`, `.DS_Store`) confuses Windows users and clutters folders | Left out by default |
| Garbled Japanese/Chinese names | Name encoding detected from the bytes (Shift_JIS, GBK, Big5, EUC-KR…), CP437 as fallback |
| Icon "lacks contrast" | High-contrast white zipper on indigo |
| Upsells and maintenance add-ons | None |

## Done in 0.1

- ZIP (Deflate, Deflate64, BZIP2, LZMA, XZ, Zstd; AES and ZipCrypto), RAR 4
  and 5 (multi-volume, encrypted names), 7z (LZMA/2, PPMd, BZIP2, Deflate,
  Zstd, AES), TAR with GZ/BZ2/XZ/ZST, and single GZ/BZ2/XZ/ZST files.
- Split pieces `.001`, `.002`… opened from any piece.
- Format from magic bytes, not the extension.
- Browse and pick files; smart single-folder output; unique names.
- Password once per batch (memory only).
- Quarantine propagation; zip-slip and unsafe symlink protection.
- Finder "Open With" and double-click (file associations), drag and drop
  onto the window, ⌘O.
- History of results.

## Next

1. Finder right-click menu (Extract here / Extract to "Name" / Extract to…)
   via a Finder Sync extension or Services menu.
2. Quick Look preview of a file inside an archive before unpacking it.
3. Low-disk mode: unpack file by file and free the archive's space as it
   goes (ReclaimArc's approach), opt-in, for archives larger than free space.
4. Move the archive to the Bin after a successful unpack (setting, off by default).
5. Creating archives: ZIP/7z, AES-256, split into pieces, leave out Mac
   clutter so Windows users see clean folders.
6. More formats: ISO, DMG contents, CPIO/XAR/PKG, legacy (StuffIt, LHA).
7. Drag files out of the contents panel straight to Finder.
