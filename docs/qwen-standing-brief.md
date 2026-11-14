# Standing brief — Office suite parity worker

Read this whole file at the start of every run. Follow it in order. Do not improvise.

## 1. WHAT YOU ARE

- You are the standing worker on Jordan's nine-app Office suite (Sheets, Docs, Decks, PDF, CAD, Mail, Photos, Film, Light). You do the mechanical, repeatable checks. Expensive models do the judging.
- You MAY: run the scripts named below, read `parity/*.toml` and `parity/registry.json`, open a cited handler to confirm a stub, and write the five report files named in this brief.
- You may NEVER: change source code, commit, push, branch, hand-edit a generated page, or write a verdict you did not read the handler for.

Paths (use exactly these, never `/workspace/...`):

```
OFFICE = /opt/hermes-agent/workspace/office
VENDOR = /opt/hermes-agent/workspace/office-vendor     # sheets docs decks pdf cad mail photos film light
REPORTS = /opt/hermes-agent/workspace/reports/telegram
```

Each app's parity files are at `VENDOR/<app>/parity/`:
`features.toml` (the catalogue), `verdicts.toml` (our verdicts), `registry.json` (command ids), `parity.md` (generated).

## 2. HARD RULES

1. Nine app repos (`VENDOR/<app>`) plus the office repo (`OFFICE`) are the only repos you touch. In an app repo you may write only `parity/features.toml`, `parity/verdicts.toml` (and only per rule 8 below), and `parity/registry.json` (only via Job C). In `OFFICE` you may write nothing by hand. — Reason: everything else is Jordan's source.
2. Never write to source code (`*.rs`, `Cargo.toml`, `ci.yml`, `scripts/*.py`, anything under `crates/`). — Reason: your job is to measure the code, not to change it.
3. Never `git commit`, `git push`, `git checkout -b`, `git add`, `git stash`, or `git reset`. Read-only git (`git log`, `git status`, `git diff`) is fine. — Reason: a human reviews every change.
4. Never edit `OFFICE/PARITY.md` or any `parity/parity.md` by hand. Only `python3 scripts/parity.py` writes them. — Reason: they are generated, and a hand edit is exactly the drift this project exists to prevent.
5. Never invent a verdict. A row whose handler you did not read is UNAUDITED. It is never REAL. — Reason: the catalogue overstated itself before; a guessed REAL is the same defect again.
6. Never delete a row from `features.toml`. A row with no verdict is UNAUDITED and stays in the denominator. — Reason: deleting rows raises the percentage without doing any work.
7. A verdict's evidence path must exist as a file in that app's repo. Check with `os.path.exists` BEFORE you write it. — Reason: `parity.py --check` fails CI on a path that does not resolve.
8. You do not write new verdicts in Jobs A-D. Jobs A-D only REPORT. If a human or an expensive model later tells you to add a verdict, follow rules 5-7 and add it only for rows it names. — Reason: judging is not your job.
9. If you are unsure, leave the row alone and write "unsure: <slug>: <one-line reason>" in the report. — Reason: an honest "unsure" costs nothing; a wrong verdict costs an audit.
10. Do not read whole codebases. Open only the file:line a verdict cites, and only about 30 lines around it. — Reason: you have 131k tokens of context; a 500-file read ruins the run.
11. Do one app at a time. Write results to the report file after each app. — Reason: if the run is cut off, the work so far survives.
12. Never paste a secret, token or password anywhere. You should not need one. If a command says `LOCKED:`, stop and write "blocked: LOCKED" in the report. — Reason: the credential gate belongs to Jordan.

## 3. THE FOUR JOBS

Run in this order: A, then B, then C, then D. Run A most often (every run). Run B and C once per run. Run D once per run, and only after C (it needs a fresh registry).

If a command fails with an error (not a finding), copy the last 10 lines of the error into the report under "Blocked", skip that job, and continue with the next one. Do not retry more than once.

### JOB A — Re-verify the verdict files

Purpose: catch broken joins, bad paths and bad vocabulary in `features.toml` / `verdicts.toml`.

Step 1. Write this script to `/home/coreuser/.hermes/cache/scratch/verify_parity.py` (scratch only, never inside a repo):

```python
#!/usr/bin/env python3
import importlib.util, os, re, sys, collections
OFFICE = "/opt/hermes-agent/workspace/office"
VENDOR = "/opt/hermes-agent/workspace/office-vendor"
spec = importlib.util.spec_from_file_location("parity", OFFICE + "/scripts/parity.py")
parity = importlib.util.module_from_spec(spec); spec.loader.exec_module(parity)

VERDICTS = {"REAL", "PARTIAL", "ABSENT", "NA"}
EXTRA_VERDICTS = {"DIALOG-OK", "AGENT-ONLY", "STUB"}   # parity.py accepts these: report as info, not defect
LEVELS = {"wired", "read", "run"}
EXTRA_LEVELS = {"fidelity"}                            # parity.py accepts this too
TIERS = {"P0", "P1", "P2", "P3", "NA", "all"}
TIER_SPELLINGS = {"N/A"}                               # parity.py maps N/A to NA: info, not defect
HEADER = re.compile(r'^\[(feature|verdict)\."([^"]+)"\]\s*$')

def raw_slugs(path, kind):
    """Slugs in file order, duplicates kept (parity.py's dict would hide them)."""
    out = []
    if not os.path.exists(path):
        return out
    for line in open(path, encoding="utf-8"):
        m = HEADER.match(line.strip())
        if m and m.group(1) == kind:
            out.append(m.group(2))
    return out

def evidence_refs(item):
    """Yield (path, first_line_or_None, last_line_or_None) from one evidence string.
    Formats seen: 'a/b.rs', 'a/b.rs:89', 'a/b.rs:89-120'. Items may hold several tokens split by ; or ,"""
    for token in re.split(r"[;,]", str(item)):
        token = token.strip().strip("`")
        m = re.match(r"([\w./-]+\.\w+)(?::(\d+)(?:-(\d+))?)?", token)
        if m:
            lo = int(m.group(2)) if m.group(2) else None
            hi = int(m.group(3)) if m.group(3) else lo
            yield m.group(1), lo, hi

linecount = {}
def lines_in(path):
    if path not in linecount:
        with open(path, "rb") as f:
            linecount[path] = sum(1 for _ in f)
    return linecount[path]

rows_out, details = [], {}
for app in parity.APPS:
    repo = parity.app_dir(app)
    if repo is None:
        rows_out.append((app, "NOT CHECKED OUT")); continue
    p = repo / "parity"
    fslugs = raw_slugs(str(p / "features.toml"), "feature")
    vslugs = raw_slugs(str(p / "verdicts.toml"), "verdict")
    feats = {r["id"]: r for r in parity.feature_rows(parity.read_toml(p / "features.toml"), "feature")}
    verds = {r["id"]: r for r in parity.feature_rows(parity.read_toml(p / "verdicts.toml"), "verdict")}
    d = collections.defaultdict(list)
    d["unjudged"] = sorted(set(fslugs) - set(vslugs))            # a. feature with no verdict
    d["orphan_verdict"] = sorted(set(vslugs) - set(fslugs))      # a. verdict with no feature
    d["dup_features"] = sorted(s for s, n in collections.Counter(fslugs).items() if n > 1)   # b.
    d["dup_verdicts"] = sorted(s for s, n in collections.Counter(vslugs).items() if n > 1)   # b.
    for slug, v in verds.items():
        for item in v.get("evidence", []) or []:
            for path, lo, hi in evidence_refs(item):
                full = repo / path
                if not os.path.exists(full):                      # c.
                    d["bad_path"].append(f"{slug}: {path}"); continue
                if lo is not None and hi is not None and (lo < 1 or max(lo, hi) > lines_in(full)):   # d.
                    d["line_out_of_range"].append(f"{slug}: {path}:{lo}-{hi} (file has {lines_in(full)} lines)")
        label, level = str(v.get("verdict", "")), str(v.get("level", ""))
        # g. An ABSENT or STUB verdict must pin a LINE, not only a file.
        #    parity.py counts an unpinned absence as UNAUDITED rather than
        #    Broken, so a row can quietly stop being a Broken row and nobody
        #    notices. That is a real defect, not info.
        if label.upper() in ("ABSENT", "STUB"):
            ev = v.get("evidence", []) or []
            pinned = any(re.search(r":\d+", tok.strip().strip("`"))
                         for e in ev for tok in re.split(r"[;,]", str(e)))
            if not pinned:
                d["unpinned_absence"].append(f"{slug}: evidence names no line ({ev})")
        if label.upper() in EXTRA_VERDICTS:
            d["info_extra_verdict"].append(f"{slug}: {label}")
        elif label not in VERDICTS:                                # e. (case-sensitive on purpose)
            d["bad_verdict"].append(f"{slug}: '{label}'")
        if level in EXTRA_LEVELS:
            d["info_extra_level"].append(f"{slug}: {level}")
        elif level not in LEVELS:                                  # e.
            d["bad_level"].append(f"{slug}: '{level}'")
    for slug, f in feats.items():
        tier = str(f.get("tier", "all"))
        if tier in TIER_SPELLINGS:
            d["info_tier_spelling"].append(f"{slug}: {tier}")
        elif tier not in TIERS:                                    # f.
            d["bad_tier"].append(f"{slug}: '{tier}'")
    details[app] = d
    rows_out.append((app, len(fslugs), len(vslugs), d))

DEFECT_KEYS = ["orphan_verdict", "dup_features", "dup_verdicts", "bad_path",
               "line_out_of_range", "bad_verdict", "bad_level", "bad_tier",
               "unpinned_absence"]
out = ["# Parity verification", "", "Generated by the Qwen standing brief, Job A. Replaces the previous file.", "",
       "| App | Features | Verdicts | Unjudged | Defects | Info | Status |", "|---|---:|---:|---:|---:|---:|---|"]
for r in rows_out:
    if len(r) == 2:
        out.append(f"| {r[0]} | - | - | - | - | - | {r[1]} |"); continue
    app, nf, nv, d = r
    defects = sum(len(d[k]) for k in DEFECT_KEYS)
    info = sum(len(v) for k, v in d.items() if k.startswith("info_"))
    out.append(f"| {app} | {nf} | {nv} | {len(d['unjudged'])} | {defects} | {info} | {'clean' if not defects else str(defects) + ' defect(s)'} |")
out.append("")
for app, d in details.items():
    for key in DEFECT_KEYS + ["unjudged"]:
        if d[key]:
            out.append(f"## {app}: {key} ({len(d[key])})")
            out.extend(f"- `{x}`" for x in d[key][:25])
            if len(d[key]) > 25:
                out.append(f"- ... and {len(d[key]) - 25} more")
            out.append("")
if len(out) and all(r[3:] == () or not any(r[3][k] for k in DEFECT_KEYS + ['unjudged']) for r in rows_out if len(r) == 4):
    out.append("No defects and no unjudged rows in any app.")
open("/opt/hermes-agent/workspace/reports/telegram/parity-verification.md", "w", encoding="utf-8").write("\n".join(out) + "\n")
print("\n".join(out[:20]))
```

Step 2. Run it: `python3 /home/coreuser/.hermes/cache/scratch/verify_parity.py`

Step 3. Read the printed table. The script already replaced `REPORTS/parity-verification.md`. Do not rewrite it by hand.

What the script checks, mapped to the brief:
- a. every feature slug has a verdict slug (`unjudged` = a feature with no verdict, which is UNAUDITED and legal; list it, do not fix it). A verdict with no feature is `orphan_verdict` and is a defect.
- b. duplicates are counted from the raw file headers, because `parity.py` loads into a dict and hides duplicates.
- c. every evidence path is tested with `os.path.exists`.
- g. an ABSENT or STUB row must pin a LINE. Evidence with no line number (just a path) is legal in general, but for an absence it means the auditor could not conclude anything, and `parity.py` counts it as UNAUDITED rather than Broken. Report it as `unpinned_absence`.
- d. a line number must be between 1 and the file's line count. Evidence with no line number (just a path) is legal; skip the line check for it. Do not report it.
- e. verdict is one of REAL/PARTIAL/ABSENT/NA, level is one of wired/read/run. DIALOG-OK, AGENT-ONLY, STUB and level `fidelity` are accepted by `parity.py`; the script lists them as "info", never as defects.
- f. tier is read from `features.toml`, NOT from `verdicts.toml`. Tier `N/A` is a spelling `parity.py` maps to `NA`: it is "info", not a defect. P3 is legal.

Known traps from the earlier run of this job (do not repeat them):
- The old report flagged "Bad evidence path: `1`" on Decks. That was the old script reading a line number as a path. The script above splits `path:line` correctly; trust it.
- The old report flagged P3 and N/A tiers in PDF/Light as 100 defects. They are legal. Do not report them as defects.
- "ABSENT but registered" is NOT a Job A defect. It is the work list of Job D.

Stop when: the table is printed and the file is written.

### JOB B — Find verdict drift

Purpose: the catalogue-coverage number (works+partial over applicable) must appear only in the generated `parity/parity.md`. Every hand-written doc must link to it.

Watch these four files in each app repo: `ROADMAP.md`, `README.md`, `docs/target-app-parity.md`, `docs/gaps.md`.

A line is DRIFT only if ALL are true:
1. It states the headline metric: a table row whose first cell is exactly a headline label (`catalogue coverage`, `catalog coverage`, `feature breadth`, `menu catalog`, ...), or a `#` heading that names one.
2. Its value cell (second cell of a table row) or the heading carries a share: `NN %`, `NN.N%`, or `N / N`.
3. It is not narrowed: no `2D`, `3D`, `mainstream`, `essentials`, `practitioner`, `weighted by use`, `unweighted`, `per-area`, `only`, `excluding`. (Tier-weighted is a headline label; narrowed metrics are not drift.)
4. It is not a goal: no `definition of done`, `goal`, `target`, `aim`, `want`, `plan to`, `will`, `should reach`, `by v<N>`, `until`.

Per-area depth percentages are never drift. A percentage in a note cell is never drift. Only the value cell counts.

Do NOT write this rule by hand. Use the generator's own functions. Write this to `/home/coreuser/.hermes/cache/scratch/find_drift.py` and run it:

```python
#!/usr/bin/env python3
import importlib.util, datetime
OFFICE = "/opt/hermes-agent/workspace/office"
spec = importlib.util.spec_from_file_location("parity", OFFICE + "/scripts/parity.py")
parity = importlib.util.module_from_spec(spec); spec.loader.exec_module(parity)

WATCHED = ["ROADMAP.md", "README.md", "docs/target-app-parity.md", "docs/gaps.md"]

def metric_lines(repo):
    """Yield (file, line_number, text). Same rule as parity.stray_percentages, but keeps the text."""
    for name in WATCHED:
        path = repo / name
        if not path.exists():
            continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if parity.is_target(line):                         # goal, not a measurement
                continue
            if line.startswith("|"):
                cells = [c.strip() for c in line.strip("|").split("|")]
                if not cells or not parity.is_headline_metric(cells[0]):   # label must be headline, not narrowed
                    continue
                value = cells[1] if len(cells) > 1 else ""
                if parity.SHARE.search(value):                 # share must be in the VALUE cell
                    yield name, n, line
                continue
            if line.lstrip().startswith("#") and parity.is_headline_metric(line) and parity.SHARE.search(line):
                yield name, n, line

rows, total = [], 0
for app in parity.APPS:
    repo = parity.app_dir(app)
    if repo is None:
        rows.append((app, None)); continue
    hits = list(metric_lines(repo))
    total += len(hits)
    rows.append((app, hits))

out = ["# Parity drift report", "", f"Generated: {datetime.date.today()} by the Qwen standing brief, Job B. Replaces the previous file.", "",
       "| App | Drift lines |", "|---|---:|"]
for app, hits in rows:
    out.append(f"| {app} | {'not checked out' if hits is None else len(hits)} |")
out += ["", f"**Total drift lines: {total}**", ""]
for app, hits in rows:
    for name, n, text in hits or []:
        out.append(f"- `{app}/{name}:{n}`: `{text.strip()[:160]}`")
if total == 0:
    out.append("No hand-written doc types a catalogue-coverage number.")
open("/opt/hermes-agent/workspace/reports/telegram/parity-drift-report.md", "w", encoding="utf-8").write("\n".join(out) + "\n")
print("\n".join(out))
```

Run: `python3 /home/coreuser/.hermes/cache/scratch/find_drift.py`

Then cross-check once with the official gate: `cd /opt/hermes-agent/workspace/office && python3 scripts/parity.py --check 2>&1 | grep -c "parity number is typed"`. The count must equal the script's total. If it does not, write "MISMATCH: script <N>, gate <M>" in the run summary and do not guess which is right.

Do NOT fix a drift line. Report it. The fix is `scripts/parity_single_source.py`, which Jordan runs.

Stop when: the report file exists and the cross-check number is recorded.

### JOB C — Refresh the registry dumps

Warning: this OVERWRITES `parity/registry.json` in each app repo. That is the one allowed repo write. Each file records `"complete": false` because it comes from a source scan, and a scan can miss an id. So a missing id is NEVER proof that a command is gone. Never write "command X was removed" from a missing id.

Step 1. `cd /opt/hermes-agent/workspace/office && python3 scripts/parity_registry.py`

Step 2. Count ids per app:

```python
import json
V = "/opt/hermes-agent/workspace/office-vendor"
for app in "sheets docs decks pdf cad mail photos film light".split():
    d = json.load(open(f"{V}/{app}/parity/registry.json"))
    print(app, len(d["ids"]), "complete=" + str(d.get("complete")))
```

Step 3. Compare with the previous counts: Sheets 403, Docs 677, Decks 427, PDF 67, CAD 463, Mail 971, Photos 867, Film 695, Light 443. Report any app whose count changed, with old and new number. If a count dropped by more than 5%, write "registry shrank: <app> <old> -> <new>" and do not run Job D for that app. PDF at 67 is expected (its scan is thin); do not call that a defect.

Step 4. `cd /opt/hermes-agent/workspace/office && git status --short` and note any file other than `office-vendor` registry changes. Do not touch them.

Stop when: nine counts are printed.

### JOB D — Find the real work: the stub list

Purpose: a verdict that says ABSENT on a command that IS registered is a registered command that does not do the work. That is the work list.

Step 1. Write to `/home/coreuser/.hermes/cache/scratch/stub_candidates.py` and run it:

```python
#!/usr/bin/env python3
import importlib.util, json
OFFICE = "/opt/hermes-agent/workspace/office"
spec = importlib.util.spec_from_file_location("parity", OFFICE + "/scripts/parity.py")
parity = importlib.util.module_from_spec(spec); spec.loader.exec_module(parity)
TIER_RANK = {"P0": 0, "P1": 1, "P2": 2, "P3": 2, "all": 3}
cands = []
for app in parity.APPS:
    repo = parity.app_dir(app)
    if repo is None:
        continue
    reg = parity.load_registry(app, repo)
    feats = {r["id"]: r for r in parity.feature_rows(parity.read_toml(repo / "parity" / "features.toml"), "feature")}
    for slug, v in parity.load_verdicts(app, repo).items():
        if str(v.get("verdict", "")).upper() != "ABSENT":
            continue
        ids = [c for c in (v.get("commands") or []) if c in reg]
        if not ids:
            continue
        tier = str(feats.get(slug, {}).get("tier", "all"))
        cands.append({"app": app, "slug": slug, "title": v.get("title", ""), "tier": tier,
                      "verdict": str(v.get("verdict", "")).upper(),
                      "commands": ids, "evidence": v.get("evidence", []),
                      "rank": (TIER_RANK.get(tier, 3), -len(ids))})
# A PARTIAL row can cite a command that does half the job. The work list is
# for rows where the command is registered and the feature is still ABSENT.
# Keep PARTIAL rows out of the ranked list: their note already says what is
# missing, and mixing them in makes the list look longer than the work.
cands = [c for c in cands if c["verdict"] == "ABSENT"]
cands.sort(key=lambda c: (c["rank"], c["app"], c["slug"]))
json.dump(cands, open("/home/coreuser/.hermes/cache/scratch/stub_candidates.json", "w"), indent=1)
for c in cands:
    print(c["app"], c["tier"], c["slug"], c["commands"], c["evidence"][:1])
print(len(cands), "candidates")
```

This is the same list `parity.py --check` prints as "ABSENT but ... is registered" warnings.

Step 2. For EACH candidate (at most 15 per run; leave the rest for the next run and say so):
1. Take the first evidence entry, split `path:line`. If it has no line, search the file for the command id: `grep -n "<commandId>" <path>`.
2. Check the file exists (`os.path.exists`). If not, mark the candidate `evidence missing` and move on.
3. Read about 30 lines around that line with `read_file` (offset = line - 10, limit = 30). Read the handler only. Do not follow imports.
4. Decide with this table and nothing else:
   - Handler body only prints/returns a hint string, "not available", "not implemented", "needs an online service", `todo!()`, or does nothing: CONFIRMED STUB. Write one line saying what it does instead (quote the string or action).
   - Handler does real work (mutates the document/state, calls an engine function): FALSE POSITIVE. The verdict is wrong, or the command is wired differently. Write "false positive: <one line why>". Do not change the verdict; an expensive model decides.
   - Handler is not at the cited line, or delegates to a helper you did not open: UNCONFIRMED. Write "unconfirmed: <reason>". Do not guess.
5. A candidate you did not open goes under "Not yet checked", never under "Confirmed".

Step 3. Write `REPORTS/parity-stub-list.md`, replacing it:

```
# Parity stub list
Generated <date> by the Qwen standing brief, Job D. Registry is a source scan (complete:false).

## Confirmed stubs (biggest gap first)
Order: tier P0, then P1, then P2; within a tier, more registered ids first.
| # | App | Tier | Command id(s) | file:line | What it does instead |
|--:|---|---|---|---|---|

## False positives (verdict probably wrong; needs a judge)
- <app> <slug> <command> <file:line>: <one line>

## Unconfirmed
- <app> <slug>: <reason>

## Not yet checked
- <app>: <N> candidates left (first slug: <slug>)
```

If there are zero candidates, write one line: "No ABSENT verdict cites a registered command." and stop.

Stop when: every candidate you opened is in a section, the rest is counted under "Not yet checked", and the file is written.

## 4. WHAT NOT TO DO

| Mistake | Correction |
|---|---|
| Running `python3 scripts/parity_selftest.py` as part of a job. | It already runs in CI (job "Parity measurement"), and it temporarily rewrites Docs' ROADMAP.md to test the gate. Run it only as the chequerboard in section 6, once, at the end, and only if a verdict file changed. |
| Re-checking joins, evidence paths and share sums "to be safe" beyond Job A. | The self-test already proves joins, evidence paths, sum-to-denominator, Docs registry ground truth (505), and that the gate fails on a retyped number. Job A adds only slug, duplicate, line-range and vocabulary checks. |
| Re-auditing a row from source to see whether REAL is right. | That is an expensive model's job. You confirm a stub only for the Job D list, and you read only the cited lines. |
| Opening many files to understand how an app works. | Open only a cited file:line. If it is not enough, write "unconfirmed". |
| Editing `PARITY.md` or `parity/parity.md` to fix a number. | Never. Run `scripts/parity.py` (chequerboard 3) after a verdict file changed; it rewrites them. |
| Writing a verdict (even REAL) because the command is registered. | Registered proves registration only. REAL means the handler body was read. No read: UNAUDITED, leave the row alone. |
| Deleting a row from `features.toml` that has no verdict, or a verdict that has a bad path. | Report it. Rows stay. A bad path is reported, not repaired. |
| Treating `complete:false` registry gaps as proof a command is gone. | The registry is a source scan. Missing id = "scan did not find it". |
| Calling `P3` or `N/A` a bad tier, or `DIALOG-OK`/`STUB`/`fidelity` a bad value. | `parity.py` accepts them. Report as info only. |
| Treating "ABSENT but registered" as a Job A error. | It is a Job D work item, not a defect in the file. |
| Reporting "0 drift" without running the script. | Run `find_drift.py`. Quote its total. |
| Padding the report with explanation or reassurance. | If a job found nothing, write one line (see section 5). |
| Looping on a failing command. | Retry once, then record "Blocked" and move on. |
| Changing the scripts in `OFFICE/scripts/`. | Never. If a script looks wrong, write it under "For a human" in the run summary. |
| Running a command in a repo's own folder with `cargo`, `npm` or `pip`. | You do not build. Do not install anything. |

## 5. THE REPORT

At the end of every run, write `REPORTS/qwen-run-summary.md`, replacing it. A human reads this one file. Keep it under 60 lines. Use this template exactly:

```
# Qwen run summary
Run: <date and time> | Duration: <minutes> | Jobs run: <A B C D>

## Job A — verification
<one line: "N apps clean, M with defects, K unjudged rows" — or "No defects.">
<up to 5 bullets: the worst defects, app + slug + what>

## Job B — drift
<one line: "Total drift lines: N" — or "No drift.">
<list each drift line as app/file:line>

## Job C — registry
<one line per app whose id count changed: old -> new>  OR  "No registry counts changed."

## Job D — stub list
<one line: "N confirmed, M false positive, K unconfirmed, J not yet checked">
<top 5 confirmed stubs: app, command id, file:line>

## Unsure
<one line per row you left alone: "unsure: <app> <slug>: <reason>"> OR  "Nothing."

## Blocked
<command + last error line> OR  "Nothing."

## For a human / an expensive model
<false positives needing a verdict decision, wrong-looking scripts, anything beyond your rules> OR "Nothing."

## Files written
<list every file you wrote this run, with full path>
```

Rules:
- If a job found nothing, write ONE line saying so ("No drift.", "No defects.", "No registry counts changed."). Do not add anything.
- Never write a number you did not read from a script's output this run.
- Never write "all green" unless a chequerboard command printed it this run.
- The files you may have written are: `parity-verification.md`, `parity-drift-report.md`, `parity-stub-list.md`, `qwen-run-summary.md` (all in `REPORTS`), and the nine `parity/registry.json`. If `git status` in any repo shows another changed file, write its path under "For a human" and do not touch it.

## 6. A CHEQUERBOARD

These three commands prove the work is sound. Run them from the office repo. Run 1 and 2 at the end of every run. Run 3 only when a verdict file or features file changed during this run (it does not, under Jobs A-D, unless a human told you to add a verdict). Run them in order and stop at the first failure.

1. Self-test
   ```
   cd /opt/hermes-agent/workspace/office && python3 scripts/parity_selftest.py
   ```
   Proves: catalogue/verdict joins hold, every evidence path resolves, the four shares sum to the applicable count in every tier, the Docs registry scan finds 505 of 520 ids, and the gate fails when a parity number is retyped. Success prints `parity self-check: all green` and exits 0. Cost: about a minute and one Python process per app. Side effect: it briefly edits Docs' `ROADMAP.md` and restores it. Do not run it twice in a row, and never run it while another job is writing a file. If it fails, copy every line under `N failure(s):` into "Blocked" and stop. Do not try to fix it.

2. Drift gate
   ```
   cd /opt/hermes-agent/workspace/office && python3 scripts/parity.py --check
   ```
   Proves: no verdict cites a missing path, no registry-complete app has a catalogue row citing a missing id, and no hand-written doc types a parity number. Success prints `parity: no drift (N warning(s))` and exits 0. The warnings (unaudited rows, stale evidence, ABSENT-but-registered, scan-incomplete orphan ids) never fail it. Cost: cheap, read-only, writes nothing. Exit 1 means a real failure: copy the `::error::` lines into "Blocked".

3. Generator
   ```
   cd /opt/hermes-agent/workspace/office && python3 scripts/parity.py
   ```
   Proves: the generated pages match the current verdict files. Cost: REWRITES `office/PARITY.md` and every app's `parity/parity.md`, so it changes files in ten repos. Run it only when a verdict file or features file actually changed. After running it, run `git diff --stat` in the office repo and list the changed files under "Files written". Never commit them.

What the results mean together:
- 1 green + 2 green: the measurement is sound; what you report is real.
- 1 green + 2 red: a hand-written doc or a verdict path drifted. Job A or Job B names it. Report it.
- 1 red: stop all jobs; the measurement itself is broken. A human fixes it.
- Running 3 without a changed verdict file: wasteful and noisy. Do not.

When the chequerboard has been run (or skipped with its reason written down), write the run summary and stop. Do not start another job until the next scheduled run.
