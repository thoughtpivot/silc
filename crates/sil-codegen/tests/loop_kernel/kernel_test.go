package main

// Replay traces for the loop kernel against the lowered examples/rfiChaseApp
// (plan.json) and the tests/fixtures/loop_digest.silc fixture, scheduled
// (digest_plan.json) and as a command (command_plan.json), and the
// tests/fixtures/loop_page_read.silc page reads (page_plan.json), and the
// tests/fixtures/loop_embed.silc embedding step (loop_embed_plan.json). The Rust test
// `loop_kernel_replay_traces` copies this file, the kernel template, and the
// plans into a temp module and runs `go test`.

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

type fakeAsker struct {
	replies []string
	calls   int
}

func (f *fakeAsker) Ask(req AskRequest) (string, error) {
	i := f.calls
	f.calls++
	if i >= len(f.replies) {
		i = len(f.replies) - 1
	}
	return f.replies[i], nil
}

const goodDraft = `{"subject":"RFI-101 is overdue","body":"Hi Dana, a reply on RFI-101 would unblock the curtain wall."}`

type harness struct {
	t     *testing.T
	dir   string
	db    *sql.DB
	k     *Kernel
	asker *fakeAsker
	clock time.Time
}

func loadTestPlan(t *testing.T) *Plan {
	t.Helper()
	return loadTestPlanFile(t, "plan.json")
}

func loadTestPlanFile(t *testing.T, file string) *Plan {
	t.Helper()
	plan, err := loadPlan(file)
	if err != nil {
		t.Fatalf("load plan: %v", err)
	}
	return plan
}

func newHarness(t *testing.T, at string) *harness {
	t.Helper()
	h := newBareHarness(t, at, "plan.json")
	h.seed("rfis", "rfi-101", map[string]any{"number": "RFI-101", "title": "Curtain wall anchor spacing", "status": "open", "due": "2026-09-28", "assignee": "Dana Ortiz", "pm": "Sam Lee", "last_reminded": ""})
	h.seed("rfis", "rfi-102", map[string]any{"number": "RFI-102", "title": "Level 3 slab penetration", "status": "open", "due": "2026-09-30", "assignee": "", "pm": "Sam Lee", "last_reminded": ""})
	h.seed("rfis", "rfi-103", map[string]any{"number": "RFI-103", "title": "Lobby finish schedule", "status": "closed", "due": "2026-09-15", "assignee": "Ira Chen", "pm": "Sam Lee", "last_reminded": ""})
	return h
}

func newBareHarness(t *testing.T, at, planFile string) *harness {
	t.Helper()
	dir := t.TempDir()
	db, err := sql.Open("sqlite", "file:"+filepath.Join(dir, "app.db")+"?_pragma=busy_timeout(5000)&_pragma=journal_mode(WAL)")
	if err != nil {
		t.Fatal(err)
	}
	db.SetMaxOpenConns(1)
	t.Cleanup(func() { db.Close() })
	h := &harness{t: t, dir: dir, db: db, asker: &fakeAsker{replies: []string{goodDraft}}}
	h.setClock(at)
	h.k = h.kernel(loadTestPlanFile(t, planFile))
	return h
}

func (h *harness) kernel(plan *Plan) *Kernel {
	h.t.Helper()
	k, err := NewKernel(h.db, plan, filepath.Join(h.dir, "plans"))
	if err != nil {
		h.t.Fatalf("new kernel: %v", err)
	}
	k.now = func() time.Time { return h.clock }
	k.asker = h.asker
	k.logf = func(format string, args ...any) { h.t.Logf("kernel: "+format, args...) }
	return k
}

func (h *harness) setClock(at string) {
	h.t.Helper()
	ts, err := time.Parse(time.RFC3339, at)
	if err != nil {
		h.t.Fatal(err)
	}
	h.clock = ts
}

func (h *harness) seed(table, id string, data map[string]any) {
	h.t.Helper()
	if _, err := h.db.Exec("INSERT INTO "+table+" (id, data) VALUES (?, ?)", id, encodeData(data)); err != nil {
		h.t.Fatal(err)
	}
}

func (h *harness) rows(table string) []map[string]any {
	h.t.Helper()
	rows, err := h.db.Query("SELECT id, data FROM " + table + " ORDER BY created_at, id")
	if err != nil {
		h.t.Fatal(err)
	}
	defer rows.Close()
	var out []map[string]any
	for rows.Next() {
		var id, data string
		if err := rows.Scan(&id, &data); err != nil {
			h.t.Fatal(err)
		}
		row := map[string]any{}
		_ = json.Unmarshal([]byte(data), &row)
		row["id"] = id
		out = append(out, row)
	}
	return out
}

func (h *harness) one(table string) map[string]any {
	h.t.Helper()
	rows := h.rows(table)
	if len(rows) != 1 {
		h.t.Fatalf("%s: want 1 row, got %d: %v", table, len(rows), rows)
	}
	return rows[0]
}

func (h *harness) runsOf(loop string) []map[string]any {
	var out []map[string]any
	for _, r := range h.rows("loop_runs") {
		if r["loop"] == loop {
			out = append(out, r)
		}
	}
	return out
}

// decide mimics the /loops inbox: the Bun PUT replaces the approval row.
func (h *harness) decide(status, by string) {
	h.t.Helper()
	apr := h.one("loop_approvals")
	id := apr["id"].(string)
	delete(apr, "id")
	apr["status"] = status
	apr["decided_by"] = by
	if _, err := h.db.Exec("UPDATE loop_approvals SET data = ? WHERE id = ?", encodeData(apr), id); err != nil {
		h.t.Fatal(err)
	}
}

func (h *harness) tick() {
	h.t.Helper()
	h.k.Tick()
}

// tickCrash runs one tick that is expected to die at a crash checkpoint.
func (h *harness) tickCrash() {
	h.t.Helper()
	crashed := false
	func() {
		defer func() {
			if r := recover(); r != nil {
				if _, ok := r.(crashSignal); !ok {
					panic(r)
				}
				crashed = true
			}
		}()
		h.k.Tick()
	}()
	if !crashed {
		h.t.Fatal("expected the kernel to crash at a checkpoint")
	}
}

func (h *harness) eventCount(runID, kind string) int {
	var n int
	_ = h.db.QueryRow("SELECT COUNT(*) FROM loop_events WHERE run_id = ? AND kind = ?", runID, kind).Scan(&n)
	return n
}

// Monday 2026-10-05 08:00 in New York.
const mondayEight = "2026-10-05T12:00:20Z"

func TestScheduleFiringWaitsForApproval(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.tick()

	runs := h.runsOf("RfiChase")
	if len(runs) != 1 {
		t.Fatalf("want one run, got %v", runs)
	}
	run := runs[0]
	if run["status"] != "waiting" {
		t.Fatalf("run should wait for the PM: %v", run)
	}
	if run["plan"] != h.k.plan.Hash {
		t.Fatalf("run must be pinned to the current plan: %v", run["plan"])
	}
	apr := h.one("loop_approvals")
	if apr["status"] != "pending" || apr["by"] != "Sam Lee" {
		t.Fatalf("approval: %v", apr)
	}
	if !strings.Contains(apr["show"].(string), "RFI-101 is overdue") {
		t.Fatalf("approval should show the draft: %v", apr["show"])
	}
	if h.asker.calls != 1 {
		t.Fatalf("silclm should be asked once (rfi-102 has no assignee), got %d", h.asker.calls)
	}
	if n := len(h.rows("reminders")); n != 0 {
		t.Fatalf("nothing is written before approval, got %d reminders", n)
	}

	// Same minute again: the run identity is the firing, so no second run.
	h.tick()
	if n := len(h.runsOf("RfiChase")); n != 1 {
		t.Fatalf("a firing starts at most one run, got %d", n)
	}
}

func TestWrongApproverIsReset(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.tick()
	h.decide("approved", "Dana Ortiz")
	h.tick()

	apr := h.one("loop_approvals")
	if apr["status"] != "pending" {
		t.Fatalf("approval by the wrong person must not count: %v", apr)
	}
	if !strings.Contains(apr["note"].(string), "Only Sam Lee") {
		t.Fatalf("approval should explain who can decide: %v", apr["note"])
	}
	if n := len(h.rows("reminders")); n != 0 {
		t.Fatalf("no reminder without the right approver, got %d", n)
	}
	if run := h.runsOf("RfiChase")[0]; run["status"] != "waiting" {
		t.Fatalf("run keeps waiting: %v", run)
	}
}

func TestApprovalCompletesRunExactlyOnce(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.tick()
	h.setClock("2026-10-05T13:10:00Z")
	h.decide("approved", "sam lee")
	h.tick()

	run := h.runsOf("RfiChase")[0]
	if run["status"] != "finished" || run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	if !strings.Contains(run["detail"].(string), "1 done, 1 skipped") {
		t.Fatalf("detail: %v", run["detail"])
	}
	rem := h.one("reminders")
	if rem["rfi"] != "rfi-101" || rem["to"] != "Dana Ortiz" || rem["subject"] != "RFI-101 is overdue" {
		t.Fatalf("reminder: %v", rem)
	}
	for _, r := range h.rows("rfis") {
		if r["id"] == "rfi-101" && r["last_reminded"] != "2026-10-05" {
			t.Fatalf("rfi-101 should be stamped with the run's day: %v", r)
		}
		if r["id"] == "rfi-101" && r["title"] != "Curtain wall anchor spacing" {
			t.Fatalf("update merges fields: %v", r)
		}
	}
	notice := h.one("loop_notices")
	if notice["to"] != "Dana Ortiz" || !strings.Contains(notice["text"].(string), "RFI-101") {
		t.Fatalf("notice: %v", notice)
	}
	if h.asker.calls != 1 {
		t.Fatalf("replay must reuse the recorded answer, asked %d times", h.asker.calls)
	}
	if apr := h.one("loop_approvals"); apr["status"] != "approved" {
		t.Fatalf("approval: %v", apr)
	}

	// Replaying a finished run changes nothing.
	if err := h.k.ResumeRun(run["id"].(string)); err != nil {
		t.Fatal(err)
	}
	if n := len(h.rows("reminders")); n != 1 {
		t.Fatalf("replay duplicated a write: %d reminders", n)
	}
	if n := len(h.rows("loop_notices")); n != 1 {
		t.Fatalf("replay duplicated a notice: %d notices", n)
	}
	if h.asker.calls != 1 {
		t.Fatalf("replay asked silclm again: %d", h.asker.calls)
	}
	if n := h.eventCount(run["id"].(string), "trace:effect_skipped"); n != 3 {
		t.Fatalf("replay should skip all 3 committed effects by receipt, skipped %d", n)
	}
}

func TestCrashAfterReserveAppliesEffectOnce(t *testing.T) {
	for _, crashAt := range []int{1, 2, 3} {
		h := newHarness(t, mondayEight)
		h.tick()
		h.decide("approved", "Sam Lee")
		seen := 0
		h.k.crash = func(point, path string) {
			if point != "after_reserve" {
				return
			}
			seen++
			if seen == crashAt {
				panic(crashSignal{point, path})
			}
		}
		h.tickCrash()
		if run := h.runsOf("RfiChase")[0]; run["status"] != "running" {
			t.Fatalf("crash %d: a crashed run stays running: %v", crashAt, run)
		}
		h.k.crash = nil
		h.tick()

		run := h.runsOf("RfiChase")[0]
		if run["status"] != "finished" || run["outcome"] != "succeeded" {
			t.Fatalf("crash %d: resumed run: %v", crashAt, run)
		}
		if n := len(h.rows("reminders")); n != 1 {
			t.Fatalf("crash %d: want exactly one reminder, got %d", crashAt, n)
		}
		if n := len(h.rows("loop_notices")); n != 1 {
			t.Fatalf("crash %d: want exactly one notice, got %d", crashAt, n)
		}
		var started int
		_ = h.db.QueryRow("SELECT COUNT(*) FROM loop_receipts WHERE status != 'committed'").Scan(&started)
		if started != 0 {
			t.Fatalf("crash %d: %d receipts left uncommitted", crashAt, started)
		}
		if h.asker.calls != 1 {
			t.Fatalf("crash %d: resume asked silclm again (%d)", crashAt, h.asker.calls)
		}
		if n := h.eventCount(run["id"].(string), "trace:effect_skipped"); n != crashAt-1 {
			t.Fatalf("crash %d: effects committed before the crash are skipped by receipt; skipped %d", crashAt, n)
		}
	}
}

func TestApprovalTimesOutAndOverlappingFiringIsSkipped(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.tick()
	// Tuesday 09:00 New York: the next firing is inside the catch-up window,
	// but Monday's run is still waiting, and its approval has expired.
	h.setClock("2026-10-06T13:00:00Z")
	h.tick()

	runs := h.runsOf("RfiChase")
	if len(runs) != 1 {
		t.Fatalf("an overlapping firing must not start a run: %v", runs)
	}
	if h.eventCount("loop:RfiChase", "skipped_overlap") != 1 {
		t.Fatal("the skipped firing should be recorded")
	}
	run := runs[0]
	if run["status"] != "finished" || !strings.Contains(run["detail"].(string), "2 skipped") {
		t.Fatalf("timed-out item is skipped: %v", run)
	}
	if apr := h.one("loop_approvals"); apr["status"] != "timed_out" {
		t.Fatalf("approval: %v", apr)
	}
	if n := len(h.rows("reminders")); n != 0 {
		t.Fatalf("no reminder after a timeout, got %d", n)
	}
}

func TestCatchUpRunsLatestFiringInsideWindow(t *testing.T) {
	h := newHarness(t, "2026-10-05T11:00:00Z") // 07:00 New York
	h.tick()
	if n := len(h.runsOf("RfiChase")); n != 0 {
		t.Fatalf("no firing yet, got %d runs", n)
	}
	h.setClock("2026-10-05T14:30:00Z") // 10:30, 2.5h late, window is 4h
	h.tick()
	runs := h.runsOf("RfiChase")
	if len(runs) != 1 {
		t.Fatalf("a missed firing inside the window runs once: %v", runs)
	}
	event := runs[0]["event"].(map[string]any)
	if event["late"] != true || !strings.HasPrefix(event["scheduled_for"].(string), "2026-10-05T08:00:00") {
		t.Fatalf("event: %v", event)
	}
}

func TestCatchUpSkipsFiringOutsideWindow(t *testing.T) {
	h := newHarness(t, "2026-10-05T11:00:00Z")
	h.tick()
	h.setClock("2026-10-05T17:30:00Z") // 13:30, 5.5h late
	h.tick()
	if n := len(h.runsOf("RfiChase")); n != 0 {
		t.Fatalf("a firing outside the catch-up window must not run, got %d", n)
	}
}

func TestWaitingRunKeepsItsPinnedPlan(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.tick()
	oldHash := h.k.plan.Hash

	// A rebuild ships a plan without RfiChase; the waiting run still finishes
	// on the plan it started with.
	next := loadTestPlan(t)
	next.Hash = strings.Repeat("b", 64)
	var kept []LoopPlan
	for _, lp := range next.Loops {
		if lp.Name != "RfiChase" {
			kept = append(kept, lp)
		}
	}
	next.Loops = kept
	h.k = h.kernel(next)

	h.decide("approved", "Sam Lee")
	h.tick()
	run := h.runsOf("RfiChase")[0]
	if run["plan"] != oldHash || run["status"] != "finished" || run["outcome"] != "succeeded" {
		t.Fatalf("run should finish on pinned plan %s: %v", oldHash[:12], run)
	}
	if n := len(h.rows("reminders")); n != 1 {
		t.Fatalf("want one reminder, got %d", n)
	}
}

func TestGateAndWhereFailClosedOnMissingFields(t *testing.T) {
	h := newHarness(t, mondayEight)
	h.seed("rfis", "rfi-104", map[string]any{"number": "RFI-104", "title": "No assignee field", "status": "open", "due": "2026-09-29", "pm": "Sam Lee"})
	h.seed("rfis", "rfi-105", map[string]any{"number": "RFI-105", "title": "No due date", "status": "open", "assignee": "Dana Ortiz", "pm": "Sam Lee"})
	h.tick()

	if h.asker.calls != 1 {
		t.Fatalf("only rfi-101 passes the gate; asked %d times", h.asker.calls)
	}
	run := h.runsOf("RfiChase")[0]
	var found string
	_ = h.db.QueryRow("SELECT value FROM loop_events WHERE run_id = ? AND kind = 'find'", run["id"]).Scan(&found)
	var overdue []map[string]any
	_ = json.Unmarshal([]byte(found), &overdue)
	var ids []string
	for _, r := range overdue {
		ids = append(ids, r["id"].(string))
	}
	if strings.Join(ids, ",") != "rfi-101,rfi-104,rfi-102" {
		t.Fatalf("overdue (ordered by due, missing due excluded): %v", ids)
	}
	if h.eventCount(run["id"].(string), "trace:gate_blocked") != 2 {
		t.Fatal("both assignee-less RFIs should be blocked by the gate")
	}
}

func TestAskRetriesThenFailsTheItem(t *testing.T) {
	h := newHarness(t, mondayEight)
	// Blank subject is a valid Str; force a type error so retries still exhaust.
	h.asker.replies = []string{"Sure! Here is a reminder.", `{"subject":"only a subject"}`, `{"subject":1,"body":"x"}`}
	h.tick()
	if h.asker.calls != 3 {
		t.Fatalf("ask retries twice after the first try, got %d calls", h.asker.calls)
	}
	run := h.runsOf("RfiChase")[0]
	if run["status"] != "finished" || run["outcome"] != "partial" {
		t.Fatalf("a failed item makes the run partial: %v", run)
	}
	if !strings.Contains(run["detail"].(string), "1 failed") {
		t.Fatalf("detail: %v", run["detail"])
	}
	if n := len(h.rows("loop_approvals")); n != 0 {
		t.Fatalf("no approval for an invalid draft, got %d", n)
	}
}

func TestManualRequests(t *testing.T) {
	h := newHarness(t, "2026-10-05T11:00:00Z")
	h.seed("loop_requests", "req-1", map[string]any{"loop": "OpenRfiDigest", "requested_by": "Sam Lee", "status": "pending", "run_id": ""})
	h.seed("loop_requests", "req-2", map[string]any{"loop": "RfiChase", "requested_by": "Sam Lee", "status": "pending", "run_id": ""})
	h.seed("loop_requests", "req-3", map[string]any{"loop": "Nope", "requested_by": "Sam Lee", "status": "pending", "run_id": ""})
	h.tick()

	runs := h.runsOf("OpenRfiDigest")
	if len(runs) != 1 || runs[0]["outcome"] != "succeeded" {
		t.Fatalf("digest run: %v", runs)
	}
	notice := h.one("loop_notices")
	if notice["text"] != "2 RFIs are open as of 2026-10-05." {
		t.Fatalf("notice: %v", notice["text"])
	}
	status := map[string]any{}
	for _, r := range h.rows("loop_requests") {
		status[r["id"].(string)] = r["status"]
	}
	if status["req-1"] != "started" || status["req-2"] != "started" || status["req-3"] != "rejected" {
		t.Fatalf("requests: %v", status)
	}
	chase := h.runsOf("RfiChase")
	if len(chase) != 1 || chase[0]["trigger"] != "manual" {
		t.Fatalf("Run now also starts a scheduled loop off schedule: %v", chase)
	}
}

func TestStopBranchWhenNothingIsOpen(t *testing.T) {
	h := newHarness(t, "2026-10-05T11:00:00Z")
	if _, err := h.db.Exec(`UPDATE rfis SET data = json_set(data, '$.status', 'closed')`); err != nil {
		t.Fatal(err)
	}
	h.seed("loop_requests", "req-1", map[string]any{"loop": "OpenRfiDigest", "requested_by": "Sam Lee", "status": "pending", "run_id": ""})
	h.tick()
	run := h.runsOf("OpenRfiDigest")[0]
	if run["outcome"] != "stopped" || run["detail"] != "no open RFIs" {
		t.Fatalf("run: %v", run)
	}
	if n := len(h.rows("loop_notices")); n != 0 {
		t.Fatalf("stop sends nothing, got %d notices", n)
	}
}

func TestCronParsing(t *testing.T) {
	c, err := parseCron("0 8 * * 1-5")
	if err != nil {
		t.Fatal(err)
	}
	ny, _ := time.LoadLocation("America/New_York")
	if !c.matches(time.Date(2026, 10, 5, 8, 0, 0, 0, ny)) {
		t.Fatal("Monday 08:00 should match")
	}
	if c.matches(time.Date(2026, 10, 4, 8, 0, 0, 0, ny)) {
		t.Fatal("Sunday should not match")
	}
	if c.matches(time.Date(2026, 10, 5, 8, 1, 0, 0, ny)) {
		t.Fatal("08:01 should not match")
	}
}

// ---------------------------------------------------------------- digest fixture

type fakeMcp struct {
	calls []map[string]any
	fail  bool
}

func (f *fakeMcp) call(server, tool string, args map[string]any, token string) (map[string]any, error) {
	f.calls = append(f.calls, map[string]any{"server": server, "tool": tool, "args": args, "token": token})
	if f.fail {
		return nil, fmt.Errorf("HTTP 503: unavailable")
	}
	path, _ := args["path"].(string)
	records := []any{
		map[string]any{"lineNumber": 1, "raw": "{...}", "parsed": map[string]any{"date": "2026-09-20", "title": "old " + path}},
		map[string]any{"lineNumber": 2, "raw": "{...}", "parsed": map[string]any{"date": "2026-10-01", "title": "recent " + path}},
	}
	return map[string]any{"text": "{}", "data": map[string]any{"records": records}}, nil
}

// The model's words are never asserted. Tests feed a spread of well-formed
// and malformed replies and check what the kernel guarantees for any of them.
const goodSummary = `{"facts":"f","focus":"g"}`
const goodAction = `{"sentence":"s"}`

var validActions = []string{
	goodAction,
	`{"sentence":"Ünïcode — and \"quotes\" survive."}`,
	"Sure, here it is:\n{\"sentence\":\"Prose around the object is ignored.\"}\nHope that helps.",
	`{"sentence":"Two\nlines"}`,
}

var invalidActions = []string{
	`{"sentence":""}`,
	`{"sentence":"   "}`,
	`{"other":"field"}`,
	`not json at all`,
	``,
}

// assertSucceededRun checks a succeeded run against the fixture's contract:
// exactly one row stamped with the run time, every Str field non-blank, and
// one notice whose text is the row's sentence.
func assertSucceededRun(t *testing.T, h *harness, at string) map[string]any {
	t.Helper()
	var row map[string]any
	for _, r := range h.rows("digests") {
		if r["at"] == at {
			if row != nil {
				t.Fatalf("two rows for one run at %s", at)
			}
			row = r
		}
	}
	if row == nil {
		t.Fatalf("no row for the run at %s: %v", at, h.rows("digests"))
	}
	for _, f := range []string{"day", "at", "sentence", "focus"} {
		if s, _ := row[f].(string); strings.TrimSpace(s) == "" {
			t.Fatalf("contract field %s is blank: %v", f, row)
		}
	}
	if !strings.HasPrefix(at, row["day"].(string)) {
		t.Fatalf("day %v is not the run's day %s", row["day"], at)
	}
	matched := 0
	for _, n := range h.rows("loop_notices") {
		if n["text"] == row["sentence"] {
			matched++
		}
	}
	if matched != 1 {
		t.Fatalf("want one notice carrying the row's sentence, got %d", matched)
	}
	return row
}

func newDigest(t *testing.T, at, plan string) (*harness, *fakeMcp) {
	t.Helper()
	h := newBareHarness(t, at, plan)
	h.asker.replies = []string{goodSummary, goodAction}
	m := &fakeMcp{}
	h.k.mcp = m.call
	t.Setenv("KB_TOKEN", "kb-secret")
	return h, m
}

func (h *harness) runNow(loop, id string) {
	h.t.Helper()
	h.seed("loop_requests", id, map[string]any{"loop": loop, "requested_by": "Reader", "status": "pending", "run_id": ""})
	h.tick()
}

type recordingAsker struct {
	fakeAsker
	prompts  []string
	contexts []string
}

func (r *recordingAsker) Ask(req AskRequest) (string, error) {
	r.prompts = append(r.prompts, req.Prompt)
	r.contexts = append(r.contexts, req.Context)
	return r.fakeAsker.Ask(req)
}

func TestMcpReadsFeedAsksAndEachRunWritesItsOwnRow(t *testing.T) {
	h, m := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
	rec := &recordingAsker{fakeAsker: fakeAsker{replies: []string{goodSummary, goodAction}}}
	h.k.asker = rec
	h.runNow("DailyDigest", "req-1")

	run := h.runsOf("DailyDigest")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 2 {
		t.Fatalf("two MCP reads, got %d calls", len(m.calls))
	}
	first := m.calls[0]
	if first["tool"] != "read_window" || first["token"] != "kb-secret" ||
		!strings.HasSuffix(first["server"].(string), "/mcp") {
		t.Fatalf("call: %v", first)
	}
	args := first["args"].(map[string]any)
	if args["path"] != "notes.jsonl" || args["limit"] != float64(6) {
		t.Fatalf("args are sent as written in the contract: %v", args)
	}
	if !strings.Contains(rec.prompts[0], "Today is 2026-10-02 (Friday)") ||
		!strings.Contains(rec.prompts[0], "2026-09-26 through 2026-10-02") ||
		!strings.Contains(rec.prompts[0], "through 2026-10-08") {
		t.Fatalf("calendar window in the prompt: %s", rec.prompts[0])
	}
	if strings.Contains(rec.contexts[0], `"raw"`) || !strings.Contains(rec.contexts[0], "recent notes.jsonl") {
		t.Fatalf(":select keeps only parsed records: %s", rec.contexts[0])
	}
	firstRow := assertSucceededRun(t, h, "2026-10-02T15:00:00Z")
	if h.one("loop_notices")["to"] != "Reader" {
		t.Fatalf("notice goes to the :to in the program: %v", h.one("loop_notices"))
	}

	// Later runs, whatever the model says, each add one run, one row, and one
	// notice, and never touch earlier rows.
	for i, reply := range validActions[1:] {
		rec.replies = []string{goodSummary, reply}
		rec.calls = 0
		at := fmt.Sprintf("2026-10-02T%02d:00:00Z", 16+i)
		h.setClock(at)
		h.runNow("DailyDigest", fmt.Sprintf("req-%d", i+2))
		if n := len(h.runsOf("DailyDigest")); n != i+2 {
			t.Fatalf("reply %q: want %d runs, got %d", reply, i+2, n)
		}
		assertSucceededRun(t, h, at)
	}
	if n := len(h.rows("digests")); n != len(validActions) {
		t.Fatalf("one row per run, got %d", n)
	}
	if again := assertSucceededRun(t, h, "2026-10-02T15:00:00Z"); again["sentence"] != firstRow["sentence"] {
		t.Fatalf("an earlier row changed: %v -> %v", firstRow, again)
	}
}

// Whatever malformed reply the model gives, the gate fails the run closed:
// no row, no notice.
func TestInvalidRepliesFailClosed(t *testing.T) {
	for _, reply := range invalidActions {
		h, _ := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
		h.asker.replies = []string{goodSummary, reply}
		h.runNow("DailyDigest", "req-1")
		run := h.runsOf("DailyDigest")[0]
		if run["outcome"] != "failed" {
			t.Fatalf("reply %q: want failed, got %v", reply, run)
		}
		if n := len(h.rows("digests")); n != 0 {
			t.Fatalf("reply %q: nothing written, got %d rows", reply, n)
		}
		if n := len(h.rows("loop_notices")); n != 0 {
			t.Fatalf("reply %q: no notice, got %d", reply, n)
		}
	}
}

func TestDailyScheduleFiresAtFiveUtc(t *testing.T) {
	h, m := newDigest(t, "2026-10-03T04:59:00Z", "digest_plan.json")
	h.tick()
	if n := len(h.runsOf("DailyDigest")); n != 0 {
		t.Fatalf("too early, got %d runs", n)
	}
	h.setClock("2026-10-03T05:00:10Z")
	h.tick()
	runs := h.runsOf("DailyDigest")
	if len(runs) != 1 || runs[0]["trigger"] != "schedule" || runs[0]["outcome"] != "succeeded" {
		t.Fatalf("05:00 UTC run: %v", runs)
	}
	if len(m.calls) != 2 {
		t.Fatalf("two MCP reads, got %d", len(m.calls))
	}
	assertSucceededRun(t, h, "2026-10-03T05:00:10Z")
}

func TestMcpReadFailsClosedWithoutToken(t *testing.T) {
	h, m := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
	t.Setenv("KB_TOKEN", "")
	h.runNow("DailyDigest", "req-1")
	run := h.runsOf("DailyDigest")[0]
	if run["outcome"] != "failed" || !strings.Contains(run["detail"].(string), "KB_TOKEN is not set") {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 0 || h.asker.calls != 0 {
		t.Fatalf("no network or model calls without a token: mcp %d, ask %d", len(m.calls), h.asker.calls)
	}
}

func TestMcpReadRetriesThenFailsWhenServerIsDown(t *testing.T) {
	h, m := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
	m.fail = true
	h.runNow("DailyDigest", "req-1")
	run := h.runsOf("DailyDigest")[0]
	if run["outcome"] != "failed" || !strings.Contains(run["detail"].(string), "HTTP 503") {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 3 {
		t.Fatalf("one try plus two retries, got %d", len(m.calls))
	}
	if n := len(h.rows("digests")); n != 0 {
		t.Fatalf("nothing written, got %d", n)
	}
	// Replay reuses the recorded failure instead of calling again.
	if err := h.k.ResumeRun(run["id"].(string)); err != nil {
		t.Fatal(err)
	}
	if len(m.calls) != 3 {
		t.Fatalf("replay must not call the server again, got %d", len(m.calls))
	}
}

func TestAskAcceptsBlankStrAndRejectsWrongTypes(t *testing.T) {
	// Blank facts is a valid Str; with a non-empty focus the digest gate still passes.
	h, _ := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
	h.asker.replies = []string{`{"facts":"","focus":"y"}`, goodAction}
	h.runNow("DailyDigest", "req-1")
	run := h.runsOf("DailyDigest")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("blank Str facts should be accepted: %v", run)
	}

	// Missing fields and non-string values still fail the Summary contract.
	for _, summary := range []string{`{"facts":"x"}`, `{"facts":1,"focus":"y"}`, `{"facts":"x","focus":{"k":1}}`} {
		h, _ := newDigest(t, "2026-10-02T15:00:00Z", "digest_plan.json")
		h.asker.replies = []string{summary, goodAction}
		h.runNow("DailyDigest", "req-1")
		run := h.runsOf("DailyDigest")[0]
		if run["outcome"] != "failed" {
			t.Fatalf("%s is not a valid Summary: %v", summary, run)
		}
		if n := len(h.rows("digests")); n != 0 {
			t.Fatalf("nothing written, got %d", n)
		}
	}
}

func TestMcpCallSpeaksStreamableHttp(t *testing.T) {
	var sawSession, sawAuth bool
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		var msg map[string]any
		_ = json.Unmarshal(body, &msg)
		switch msg["method"] {
		case "initialize":
			w.Header().Set("Mcp-Session-Id", "sess-1")
			w.Header().Set("Content-Type", "application/json")
			fmt.Fprint(w, `{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{}}}`)
		case "notifications/initialized":
			w.WriteHeader(http.StatusAccepted)
		case "tools/call":
			sawSession = r.Header.Get("Mcp-Session-Id") == "sess-1"
			sawAuth = r.Header.Get("Authorization") == "Bearer tok"
			params := msg["params"].(map[string]any)
			w.Header().Set("Content-Type", "text/event-stream")
			result := map[string]any{
				"content":           []any{map[string]any{"type": "text", "text": "two records"}},
				"structuredContent": map[string]any{"records": []any{map[string]any{"parsed": map[string]any{"tool": params["name"]}}}},
			}
			b, _ := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": 2, "result": result})
			fmt.Fprintf(w, "event: message\ndata: %s\n\n", b)
		}
	}))
	defer srv.Close()

	res, err := mcpCall(srv.URL, "kb_search", map[string]any{"query": "x"}, "tok")
	if err != nil {
		t.Fatal(err)
	}
	if !sawSession || !sawAuth {
		t.Fatalf("session %v auth %v", sawSession, sawAuth)
	}
	if res["text"] != "two records" {
		t.Fatalf("text: %v", res["text"])
	}
	sel := selectPath(res["data"], "records.parsed.tool")
	if fmt.Sprint(sel) != "[kb_search]" {
		t.Fatalf("select: %v", sel)
	}

	errSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "nope", http.StatusUnauthorized)
	}))
	defer errSrv.Close()
	if _, err := mcpCall(errSrv.URL, "kb_search", nil, ""); err == nil || !strings.Contains(err.Error(), "HTTP 401") {
		t.Fatalf("want HTTP 401, got %v", err)
	}
}

func TestCheckContractTakesFirstObjectAndAcceptsBlankStr(t *testing.T) {
	fields := []FieldSpec{{Name: "first", Type: "Str"}, {Name: "fourth", Type: "Str"}}
	reply := "Sure.\n{\"first\":\"Ship the release\",\"fourth\":\"Book the room\"}\nJustification: the {focus} is real."
	got, err := checkContract(fields, reply)
	if err != nil {
		t.Fatal(err)
	}
	if got["first"] != "Ship the release" || got["fourth"] != "Book the room" {
		t.Fatalf("got %v", got)
	}
	blank := "{\"first\":\"Ship the release\",\"fourth\":\"\"}"
	got, err = checkContract(fields, blank)
	if err != nil {
		t.Fatalf("blank Str should be valid, got %v", err)
	}
	if got["first"] != "Ship the release" || got["fourth"] != "" {
		t.Fatalf("blank Str got %v", got)
	}
	whitespace := "{\"first\":\"  \",\"fourth\":\"Book the room\"}"
	got, err = checkContract(fields, whitespace)
	if err != nil {
		t.Fatalf("whitespace-only Str should be valid, got %v", err)
	}
	if got["first"] != "" || got["fourth"] != "Book the room" {
		t.Fatalf("whitespace-only Str got %v", got)
	}
	for _, tc := range []struct {
		reply string
		want  string
	}{
		{`{"first":1,"fourth":"ok"}`, `field "first" must be Str, got number`},
		{`{"first":{"a":1},"fourth":"ok"}`, `field "first" must be Str, got object`},
		{`{"first":["a"],"fourth":"ok"}`, `field "first" must be Str, got array`},
		{`{"first":true,"fourth":"ok"}`, `field "first" must be Str, got bool`},
		{`{"first":null,"fourth":"ok"}`, `field "first" must be Str, got null`},
	} {
		_, err := checkContract(fields, tc.reply)
		if err == nil || err.Error() != tc.want {
			t.Fatalf("reply %s: want %q, got %v", tc.reply, tc.want, err)
		}
	}
	if _, err := checkContract(fields, "no object here"); err == nil {
		t.Fatal("expected a missing-object error")
	}
}

// ---------------------------------------------------------------- command programs

// RunOnce is `silc main.silc` for a command program: every manual loop runs
// once, the run's notices go to the writer, and the runs persist like any other.
func TestRunOnceRunsManualLoopsAndPrintsNotices(t *testing.T) {
	h, m := newDigest(t, "2026-10-03T21:00:00Z", "command_plan.json")
	var narration []string
	h.k.progress = func(format string, args ...any) {
		narration = append(narration, fmt.Sprintf(format, args...))
	}

	for i, reply := range validActions {
		narration = nil
		at := fmt.Sprintf("2026-10-03T%02d:00:00Z", 10+i)
		h.setClock(at)
		h.asker.replies = []string{goodSummary, reply}
		h.asker.calls = 0
		before := len(m.calls)

		var out strings.Builder
		ok, err := h.k.RunOnce("Reader", &out)
		if err != nil || !ok {
			t.Fatalf("reply %q: RunOnce ok=%v err=%v", reply, ok, err)
		}
		runs := h.runsOf("DailyDigest")
		if len(runs) != i+1 {
			t.Fatalf("one run per invocation, got %d after %d", len(runs), i+1)
		}
		var run map[string]any
		for _, r := range runs {
			if r["started_at"] == at {
				run = r
			}
		}
		if run == nil || run["outcome"] != "succeeded" || run["trigger"] != "manual" ||
			!strings.HasPrefix(run["identity"].(string), "command:") {
			t.Fatalf("run: %v", run)
		}
		if ev, _ := run["event"].(map[string]any); ev["requested_by"] != "Reader" {
			t.Fatalf("event: %v", run["event"])
		}
		if len(m.calls)-before != 2 {
			t.Fatalf("two MCP reads per invocation, got %d", len(m.calls)-before)
		}
		row := assertSucceededRun(t, h, at)
		// stdout is exactly this run's notice text and nothing else.
		if out.String() != row["sentence"].(string)+"\n" {
			t.Fatalf("stdout %q is not the run's notice %q", out.String(), row["sentence"])
		}
		// The run narrates each step as it goes (this goes to stderr in silc).
		joined := strings.Join(narration, "\n")
		for _, want := range []string{
			"DailyDigest: reading notes (mcp::call read_window)", "DailyDigest: read notes ok",
			"DailyDigest: reading tasks", "DailyDigest: asking the model for Summary (summary)",
			"DailyDigest: ask summary ok", "DailyDigest: asking the model for Action (action)",
			"DailyDigest: gate passed", "DailyDigest: wrote one digests row", "DailyDigest: notice posted to Reader",
		} {
			if !strings.Contains(joined, want) {
				t.Fatalf("narration missing %q:\n%s", want, joined)
			}
		}
	}

	// Malformed replies and a down server both print nothing and report not ok.
	runs := len(h.runsOf("DailyDigest"))
	for i, reply := range invalidActions {
		h.setClock(fmt.Sprintf("2026-10-04T%02d:00:00Z", 10+i))
		h.asker.replies = []string{goodSummary, reply}
		h.asker.calls = 0
		var out strings.Builder
		ok, err := h.k.RunOnce("Reader", &out)
		if err != nil || ok || out.String() != "" {
			t.Fatalf("reply %q: ok=%v err=%v out=%q", reply, ok, err, out.String())
		}
	}
	m.fail = true
	h.setClock("2026-10-05T10:00:00Z")
	var out strings.Builder
	if ok, err := h.k.RunOnce("Reader", &out); err != nil || ok || out.String() != "" {
		t.Fatalf("server down: ok=%v err=%v out=%q", ok, err, out.String())
	}
	if n := len(h.rows("digests")); n != len(validActions) {
		t.Fatalf("failed runs write nothing: %d rows", n)
	}
	if n := len(h.runsOf("DailyDigest")); n != runs+len(invalidActions)+1 {
		t.Fatalf("failed invocations are still recorded as runs, got %d", n)
	}
}

func TestTrimOldestKeepsNewestItemsOfLargestList(t *testing.T) {
	list := []any{[]any{"a1", "a2", "a3"}, []any{"b1"}}
	got := trimOldest(list)
	if fmt.Sprint(got) != "[[a2 a3] [b1]]" {
		t.Fatalf("got %v", got)
	}
	if fmt.Sprint(trimOldest([]any{"x", "y"})) != "[y]" {
		t.Fatal("flat lists drop the first item")
	}
}

func TestKernelDBLockRefusesSecondLiveKernel(t *testing.T) {
	dir := t.TempDir()
	dbPath := filepath.Join(dir, "app.db")
	first, err := acquireKernelDBLock(dbPath)
	if err != nil {
		t.Fatalf("first lock: %v", err)
	}
	defer first.Close()

	_, err = acquireKernelDBLock(dbPath)
	if err == nil {
		t.Fatal("expected second live kernel to be refused")
	}
	if !strings.Contains(err.Error(), "live loop kernel") {
		t.Fatalf("unexpected refuse error: %v", err)
	}
}

func TestKernelDBLockAllowsRestartAfterCleanRelease(t *testing.T) {
	dir := t.TempDir()
	dbPath := filepath.Join(dir, "app.db")
	first, err := acquireKernelDBLock(dbPath)
	if err != nil {
		t.Fatalf("first lock: %v", err)
	}
	first.Close()

	second, err := acquireKernelDBLock(dbPath)
	if err != nil {
		t.Fatalf("restart after clean release: %v", err)
	}
	second.Close()
}

func TestKernelDBLockReleasedWhenHolderExits(t *testing.T) {
	dir := t.TempDir()
	dbPath := filepath.Join(dir, "data", "app.db")
	lockPath := dbPath + ".kernel.lock"

	// Simulate a crashed previous kernel: write a stale PID file without holding flock.
	if err := os.MkdirAll(filepath.Dir(dbPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(lockPath, []byte("1\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	got, err := acquireKernelDBLock(dbPath)
	if err != nil {
		t.Fatalf("stale lock file must not block restart: %v", err)
	}
	got.Close()
}

// ---------------------------------------------------------------- loop::fallback (THO-559)

func TestFallbackTriesReadsInOrderAndBindsTheFirstAccepted(t *testing.T) {
	h := newBareHarness(t, "2026-10-05T10:00:00Z", "fallback_plan.json")
	h.seed("deps", "a", map[string]any{"name": "alpha", "changelog": "https://a.test/changelog", "releases": "https://a.test/releases"})
	h.seed("deps", "b", map[string]any{"name": "beta", "changelog": "https://b.test/changelog", "releases": "https://b.test/releases"})
	h.seed("deps", "c", map[string]any{"name": "gamma", "changelog": "https://c.test/changelog", "releases": "https://c.test/releases"})
	h.seed("deps", "d", map[string]any{"name": "delta", "changelog": "https://d.test/changelog", "releases": "https://d.test/releases"})

	var fetched []string
	h.k.fetch = func(url string) (Page, error) {
		fetched = append(fetched, url)
		switch url {
		case "https://a.test/changelog":
			return Page{URL: url, Status: 200, Title: "alpha changelog", Text: "1.2.0 fixes"}, nil
		case "https://b.test/changelog":
			return Page{}, fmt.Errorf("GET %s returned 404", url)
		case "https://b.test/releases":
			return Page{URL: url, Status: 200, Title: "beta releases", Text: "2.0.0"}, nil
		case "https://c.test/changelog":
			return Page{URL: url, Status: 200, Title: "gamma", Text: ""}, nil
		case "https://c.test/releases":
			return Page{URL: url, Status: 204, Title: "gamma releases", Text: "x"}, nil
		}
		return Page{}, fmt.Errorf("GET %s returned 500", url)
	}
	var narration []string
	h.k.progress = func(format string, args ...any) {
		narration = append(narration, fmt.Sprintf(format, args...))
	}

	var out strings.Builder
	ok, err := h.k.RunOnce("Reader", &out)
	if err != nil || !ok {
		t.Fatalf("RunOnce ok=%v err=%v", ok, err)
	}
	run := h.runsOf("ReadDeps")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	if got := strings.Join(fetched, ","); got != strings.Join([]string{
		"https://a.test/changelog",
		"https://b.test/changelog", "https://b.test/releases",
		"https://c.test/changelog", "https://c.test/releases",
		"https://d.test/changelog", "https://d.test/releases",
	}, ",") {
		t.Fatalf("attempts run in order and stop at the first accepted: %v", fetched)
	}
	if out.String() != "alpha: alpha changelog\nbeta: beta releases\n" {
		t.Fatalf("only accepted pages reach the notices: %q", out.String())
	}
	runID := run["id"].(string)
	if n := h.eventCount(runID, "trace:fallback_accepted"); n != 2 {
		t.Fatalf("two dependencies accepted an attempt, got %d", n)
	}
	if n := h.eventCount(runID, "trace:fallback_exhausted"); n != 2 {
		t.Fatalf("gamma (rejected by :accept) and delta (every read failed) exhaust the fallback, got %d", n)
	}
	if n := h.eventCount(runID, "trace:skipped"); n != 2 {
		t.Fatalf("exhausted fallbacks run the otherwise block, which skips the item, got %d", n)
	}
	if n := h.eventCount(runID, "read"); n != len(fetched) {
		t.Fatalf("every attempt is recorded once, got %d for %d fetches", n, len(fetched))
	}
	if detail, _ := run["detail"].(string); !strings.Contains(detail, "2 done") || !strings.Contains(detail, "2 skipped") {
		t.Fatalf("run detail: %v", run["detail"])
	}
	joined := strings.Join(narration, "\n")
	for _, want := range []string{"reading page, attempt 1", "attempt 2 accepted", "no attempt accepted"} {
		if !strings.Contains(joined, want) {
			t.Fatalf("narration missing %q:\n%s", want, joined)
		}
	}

	// Replay reuses every recorded attempt and never refetches.
	before := len(fetched)
	if err := h.k.ResumeRun(runID); err != nil {
		t.Fatal(err)
	}
	if len(fetched) != before {
		t.Fatalf("replay must not refetch, got %d new fetches", len(fetched)-before)
	}
	if n := len(h.rows("loop_notices")); n != 2 {
		t.Fatalf("replay posts no second notice, got %d", n)
	}
}

// ---------------------------------------------------------------- embeddings

type fakeEmbedder struct {
	dim     int
	embeds  int
	texts   []string
	nearest []map[string]any
}

func (f *fakeEmbedder) Embed(texts []string) ([][]float64, error) {
	f.embeds++
	f.texts = append(f.texts, texts...)
	out := make([][]float64, len(texts))
	for i, text := range texts {
		vec := make([]float64, f.dim)
		vec[len(text)%f.dim] = 1
		out[i] = vec
	}
	return out, nil
}

func (f *fakeEmbedder) Nearest(table, needle string, limit int) ([]map[string]any, error) {
	if len(f.nearest) > limit {
		return f.nearest[:limit], nil
	}
	return f.nearest, nil
}

func newEmbedHarness(t *testing.T) (*harness, *fakeEmbedder) {
	t.Helper()
	h := newBareHarness(t, "2026-10-05T12:00:00Z", "embed_plan.json")
	if h.k.plan.Embedding == nil {
		t.Fatal("embed_plan.json has no embedding section")
	}
	e := &fakeEmbedder{dim: h.k.plan.Embedding.Dimension}
	h.k.embedder = e
	return h, e
}

func (h *harness) eventsMentioning(text string) int {
	var n int
	_ = h.db.QueryRow("SELECT COUNT(*) FROM loop_events WHERE value LIKE ?", "%"+text+"%").Scan(&n)
	return n
}

func TestLoopWriteEmbedsFeedWithoutModelCallsOrLoggedVectors(t *testing.T) {
	h, e := newEmbedHarness(t)
	h.runNow("IndexFiles", "req-1")
	row := h.one("files")
	vec, ok := row["vector_embedding"].([]any)
	if !ok || len(vec) != 384 {
		t.Fatalf("want a 384-float vector_embedding, got %v", row["vector_embedding"])
	}
	if e.embeds != 1 || !strings.HasPrefix(e.texts[0], "Daily notes for ") {
		t.Fatalf("want one embed of the feed field, got %d: %v", e.embeds, e.texts)
	}
	if h.asker.calls != 0 {
		t.Fatalf("embeds must never call silclm; asker called %d times", h.asker.calls)
	}
	if n := h.eventsMentioning("vector_embedding"); n != 0 {
		t.Fatalf("vectors must stay out of loop_events; %d events mention one", n)
	}
}

func TestLoopWriteWithoutEmbedderStoresRowWithoutVector(t *testing.T) {
	h, _ := newEmbedHarness(t)
	h.k.embedder = nil
	h.runNow("IndexFiles", "req-1")
	row := h.one("files")
	if _, has := row["vector_embedding"]; has {
		t.Fatalf("no embed worker: row should wait for the sweep, got %v", row)
	}
}

func TestFindStripsVectorsFromRecordedRows(t *testing.T) {
	h, _ := newEmbedHarness(t)
	h.seed("files", "f-1", map[string]any{"name": "a.txt", "text_excerpt": "alpha", "vector_embedding": []any{1.0, 0.0}})
	run := &Run{ID: "run-find", Data: map[string]any{}}
	c := &runCtx{k: h.k, plan: h.k.plan, lp: &h.k.plan.Loops[0], run: run, recorded: map[string]string{}, items: map[string]int{}}
	sc := &scope{vars: map[string]any{}}
	st := Step{Op: "find", As: "all", Resource: "Files", Table: "files", Max: 10}
	if out := c.step(&st, sc, "0", false); out.s != sigNone {
		t.Fatalf("find failed: %v", out.reason)
	}
	rows := sc.vars["all"].([]any)
	if len(rows) != 1 {
		t.Fatalf("want 1 row, got %v", rows)
	}
	if _, has := rows[0].(map[string]any)["vector_embedding"]; has {
		t.Fatal("find must not carry vectors into scope or loop_events")
	}
	if n := h.eventsMentioning("vector_embedding"); n != 0 {
		t.Fatalf("%d events mention vector_embedding", n)
	}
}

func TestEmbedStepIsRecomputedOnReplay(t *testing.T) {
	h, e := newEmbedHarness(t)
	run := &Run{ID: "run-embed", Data: map[string]any{}}
	st := Step{Op: "embed", As: "vec", From: &Expr{K: "str", S: "hello"}}
	for pass := 1; pass <= 2; pass++ {
		c := &runCtx{k: h.k, plan: h.k.plan, lp: &h.k.plan.Loops[0], run: run, recorded: map[string]string{}, items: map[string]int{}}
		if err := c.loadEvents(); err != nil {
			t.Fatal(err)
		}
		sc := &scope{vars: map[string]any{}}
		if out := c.step(&st, sc, "0", false); out.s != sigNone {
			t.Fatalf("pass %d: embed failed: %v", pass, out.reason)
		}
		if vec := sc.vars["vec"].([]any); len(vec) != 384 {
			t.Fatalf("pass %d: want 384 floats, got %d", pass, len(vec))
		}
		if e.embeds != pass {
			t.Fatalf("pass %d: want %d embeds (replay recomputes), got %d", pass, pass, e.embeds)
		}
	}
	if n := h.eventCount("run-embed", "embed"); n != 0 {
		t.Fatalf("embed results must not be recorded; got %d events", n)
	}
	if h.asker.calls != 0 {
		t.Fatalf("embed called silclm %d times", h.asker.calls)
	}
}

func TestNearestStepRanksThroughEmbedderAndSkipsBlankNeedles(t *testing.T) {
	h, e := newEmbedHarness(t)
	e.nearest = []map[string]any{
		{"id": "f-1", "name": "best", "vector_embedding": []any{1.0}},
		{"id": "f-2", "name": "next"},
	}
	run := &Run{ID: "run-near", Data: map[string]any{}}
	c := &runCtx{k: h.k, plan: h.k.plan, lp: &h.k.plan.Loops[0], run: run, recorded: map[string]string{}, items: map[string]int{}}
	sc := &scope{vars: map[string]any{}}
	st := Step{Op: "nearest", As: "hits", Resource: "Files", Table: "files", From: &Expr{K: "str", S: "notes"}, Max: 1}
	if out := c.step(&st, sc, "0", false); out.s != sigNone {
		t.Fatalf("nearest failed: %v", out.reason)
	}
	hits := sc.vars["hits"].([]any)
	if len(hits) != 1 || hits[0].(map[string]any)["name"] != "best" {
		t.Fatalf("want the single best row, got %v", hits)
	}
	if _, has := hits[0].(map[string]any)["vector_embedding"]; has {
		t.Fatal("nearest rows must not carry vectors")
	}
	blank := Step{Op: "nearest", As: "none", Resource: "Files", Table: "files", From: &Expr{K: "str", S: "  "}, Max: 5}
	if out := c.step(&blank, sc, "1", false); out.s != sigNone {
		t.Fatalf("blank nearest failed: %v", out.reason)
	}
	if got := sc.vars["none"].([]any); len(got) != 0 {
		t.Fatalf("blank needle should return no rows, got %v", got)
	}
}



func TestAskFromSeveralBindingsIsLabelledByName(t *testing.T) {
	sc := &scope{vars: map[string]any{
		"release": map[string]any{"version": "1.2"},
		"seen":    []any{"a", "b"},
		"kb":      map[string]any{"data": "notes"},
	}}
	list := func(items ...Expr) *Expr { return &Expr{K: "list", Items: items} }
	v := func(name string) Expr { return Expr{K: "var", Name: name} }
	member := Expr{K: "member", Base: &Expr{K: "var", Name: "kb"}, Field: "data"}

	got := askContext(list(v("release"), v("seen")), sc)
	if got != `{"release":{"version":"1.2"},"seen":["a","b"]}` {
		t.Fatalf("labelled context: %s", got)
	}
	got = askContext(list(v("release"), member, v("release")), sc)
	if got != `{"release":{"version":"1.2"},"kb.data":"notes","release#2":{"version":"1.2"}}` {
		t.Fatalf("paths and repeats: %s", got)
	}
	if got := askContext(list(v("seen")), sc); got != `[["a","b"]]` {
		t.Fatalf("a one-element list stays a list: %s", got)
	}
	if got := askContext(&Expr{K: "var", Name: "release"}, sc); got != `{"version":"1.2"}` {
		t.Fatalf("single binding is unchanged: %s", got)
	}
}

func TestAskFromSeveralBindingsKeepsEveryLabelInTheSlot(t *testing.T) {
	long := make([]any, 0, 400)
	for i := 0; i < 400; i++ {
		long = append(long, fmt.Sprintf("row-%03d-%s", i, strings.Repeat("x", 40)))
	}
	sc := &scope{vars: map[string]any{"a": long, "b": []any{"keep-1", "keep-2"}}}
	e := &Expr{K: "list", Items: []Expr{{K: "var", Name: "a"}, {K: "var", Name: "b"}}}
	got := askContext(e, sc)
	if len(got) > maxAskContext {
		t.Fatalf("context is %d bytes, over %d", len(got), maxAskContext)
	}
	var decoded map[string]any
	if err := json.Unmarshal([]byte(got), &decoded); err != nil {
		t.Fatalf("trimmed context is not valid JSON: %v", err)
	}
	if b, _ := decoded["b"].([]any); len(b) != 2 {
		t.Fatalf("the short binding must survive trimming: %v", decoded["b"])
	}
	a, _ := decoded["a"].([]any)
	if len(a) == 0 || a[len(a)-1] != long[399] {
		t.Fatalf("trimming drops the oldest rows first")
	}
}

// ---------------------------------------------------------------- page reads (THO-554)

const changelogHTML = `<html><head><title>silc changelog</title></head><body>
<nav>Home Docs Blog</nav>
<article><h2>v1.4.0</h2><p>Faster   builds.</p><script>track()</script></article>
<article><h2>v1.3.0</h2><p>Older.</p></article>
</body></html>`

type fakePages struct {
	html  map[string]string
	calls []string
}

func (f *fakePages) fetch(url string) (Page, error) {
	f.calls = append(f.calls, url)
	body, ok := f.html[url]
	if !ok {
		return Page{}, fmt.Errorf("GET %s returned 404", url)
	}
	return Page{URL: url, Status: 200, Title: "silc changelog", Text: "whole page text", HTML: body}, nil
}

func newPageReads(t *testing.T, html map[string]string) (*harness, *fakePages) {
	t.Helper()
	h := newBareHarness(t, "2026-10-02T15:00:00Z", "page_plan.json")
	p := &fakePages{html: html}
	h.k.fetch = p.fetch
	return h, p
}

func TestPageReadSelectsIntoContractAndReplays(t *testing.T) {
	url := "https://example.test/silc/CHANGELOG"
	h, p := newPageReads(t, map[string]string{url: changelogHTML})
	h.seed("dependencies", "silc", map[string]any{"name": "silc", "changelog": url})
	h.runNow("ReleaseWatch", "req-1")
	run := h.runsOf("ReleaseWatch")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	notice := h.one("loop_notices")
	if notice["text"] != "silc v1.4.0: v1.4.0 Faster builds." {
		t.Fatalf(":select keeps the first <article> without nav or script, :fields fills version: %v", notice)
	}
	if notice["key"] != "silc:v1.4.0" {
		t.Fatalf("typed field in the key: %v", notice)
	}
	if len(p.calls) != 1 {
		t.Fatalf("one fetch, got %d", len(p.calls))
	}
	var recorded string
	_ = h.db.QueryRow("SELECT value FROM loop_events WHERE run_id = ? AND kind = 'read'", run["id"]).Scan(&recorded)
	if strings.Contains(recorded, "<article>") || !strings.Contains(recorded, `"version":"v1.4.0"`) {
		t.Fatalf("the read records the selected row, not raw html: %s", recorded)
	}
	if err := h.k.ResumeRun(run["id"].(string)); err != nil {
		t.Fatal(err)
	}
	if len(p.calls) != 1 {
		t.Fatalf("replay reuses the recorded selection instead of fetching, got %d fetches", len(p.calls))
	}
	if n := len(h.rows("loop_notices")); n != 1 {
		t.Fatalf("replay duplicated a notice: %d", n)
	}
}

func TestPageReadFieldWithoutMatchIsUnknownAndGateSkips(t *testing.T) {
	url := "https://example.test/nover"
	h, _ := newPageReads(t, map[string]string{url: `<html><body><article><p>No heading.</p></article></body></html>`})
	h.seed("dependencies", "nover", map[string]any{"name": "nover", "changelog": url})
	h.runNow("ReleaseWatch", "req-1")
	if n := len(h.rows("loop_notices")); n != 0 {
		t.Fatalf("an unknown version must not reach the notice: %v", h.rows("loop_notices"))
	}
}

func TestPageSelectMatchingNothingFailsTheItemWithoutRetry(t *testing.T) {
	url := "https://example.test/plain"
	h, p := newPageReads(t, map[string]string{url: `<html><body><p>No article here.</p></body></html>`})
	h.seed("dependencies", "plain", map[string]any{"name": "plain", "changelog": url})
	h.runNow("ReleaseWatch", "req-1")
	run := h.runsOf("ReleaseWatch")[0]
	if run["outcome"] == "succeeded" {
		t.Fatalf("a selection miss must not succeed: %v", run)
	}
	if len(p.calls) != 1 {
		t.Fatalf("a selection miss is not a fetch failure; want 1 fetch, got %d", len(p.calls))
	}
	if n := len(h.rows("loop_notices")); n != 0 {
		t.Fatalf("no notice, got %d", n)
	}
}

func TestPageReadIntoTakesTitleAndSelectedText(t *testing.T) {
	url := "https://example.test/post"
	h, _ := newPageReads(t, map[string]string{url: changelogHTML})
	h.seed("sources", "post", map[string]any{"url": url})
	h.runNow("ArticleRead", "req-1")
	notice := h.one("loop_notices")
	if notice["text"] != "silc changelog: v1.4.0 Faster builds." {
		t.Fatalf("title from the page, text from the selection: %v", notice)
	}
}

func TestShapePageWithoutSelectOrIntoKeepsThePage(t *testing.T) {
	page := Page{URL: "https://x.test", Status: 200, Title: "t", Text: "all", HTML: "<p>all</p>"}
	res := shapePage(&Step{}, nil, page)
	if got, ok := res["page"].(Page); !ok || got.Text != "all" {
		t.Fatalf("plain page read is unchanged: %v", res)
	}
	res = shapePage(&Step{Select: "p"}, nil, Page{HTML: "<div><p>one <b>two</b></p></div>", Text: "x"})
	if got := res["page"].(Page); got.Text != "one two" {
		t.Fatalf(":select without :into narrows text: %v", got)
	}
}

func TestFallbackMovesPastASelectionMissToTheNextPageRead(t *testing.T) {
	url := "https://example.test/main-only"
	h, p := newPageReads(t, map[string]string{url: `<html><body><main><h1>v3.0.0</h1><p>Main only.</p></main></body></html>`})
	h.seed("dependencies", "mainonly", map[string]any{"name": "mainonly", "changelog": url})
	h.runNow("ReleaseFallback", "req-1")
	run := h.runsOf("ReleaseFallback")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	if notice := h.one("loop_notices"); notice["text"] != "mainonly v3.0.0" {
		t.Fatalf("the second attempt's :select and :fields fill the row: %v", notice)
	}
	if n := h.eventCount(run["id"].(string), "trace:fallback_rejected"); n != 1 {
		t.Fatalf("the <article> miss rejects the first attempt, got %d rejections", n)
	}
	if len(p.calls) != 2 {
		t.Fatalf("one fetch per attempt, got %d", len(p.calls))
	}
	if err := h.k.ResumeRun(run["id"].(string)); err != nil {
		t.Fatal(err)
	}
	if len(p.calls) != 2 || len(h.rows("loop_notices")) != 1 {
		t.Fatalf("replay reuses both recorded attempts: %d fetches, %d notices", len(p.calls), len(h.rows("loop_notices")))
	}
}

// --------------------------------------------- embedding steps (THO-552)

// The tests/fixtures/loop_embed.silc ReleaseIndex loop (loop_embed_plan.json):
// read a changelog, embed its text with loop::let + tensor::infer, and write
// the vector into an Emb384 field.
func TestLoopEmbedStepWritesVectorAndReplayRecomputesIt(t *testing.T) {
	url := "https://example.test/silc/CHANGELOG"
	h := newBareHarness(t, "2026-10-02T15:00:00Z", "loop_embed_plan.json")
	if h.k.plan.Embedding == nil || h.k.plan.Embedding.Dimension != 384 {
		t.Fatalf("loop_embed_plan.json needs a 384-d embedding section, got %+v", h.k.plan.Embedding)
	}
	p := &fakePages{html: map[string]string{url: changelogHTML}}
	h.k.fetch = p.fetch
	e := &fakeEmbedder{dim: h.k.plan.Embedding.Dimension}
	h.k.embedder = e
	h.seed("dependencies", "silc", map[string]any{"name": "silc", "changelog": url})
	h.runNow("ReleaseIndex", "req-1")
	run := h.runsOf("ReleaseIndex")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	const text = "v1.4.0 Faster builds."
	if e.embeds != 1 || e.texts[0] != text {
		t.Fatalf("want one embed of the selected release text, got %d: %q", e.embeds, e.texts)
	}
	row := h.one("incidents")
	vec, ok := row["vector_embedding"].([]any)
	if !ok || len(vec) != 384 || vec[len(text)%384] != 1.0 {
		t.Fatalf("want the embedder's 384-float vector in the row, got %v", row["vector_embedding"])
	}
	if row["title"] != "v1.4.0" {
		t.Fatalf("row: %v", row)
	}
	if h.asker.calls != 0 {
		t.Fatalf("embeds must never call silclm; asker called %d times", h.asker.calls)
	}
	runID := run["id"].(string)
	if n := h.eventCount(runID, "embed"); n != 0 {
		t.Fatalf("embed results must not be recorded; got %d events", n)
	}
	if n := h.eventsMentioning("vector_embedding"); n != 0 {
		t.Fatalf("vectors must stay out of loop_events; %d events mention one", n)
	}
	if n := h.eventsMentioning("[0,0,0,0"); n != 0 {
		t.Fatalf("no event may carry a vector; %d do", n)
	}

	// Replay: the read comes from the event log, the embed is recomputed from
	// that recorded text, and the committed effects are not repeated.
	if err := h.k.ResumeRun(runID); err != nil {
		t.Fatal(err)
	}
	if len(p.calls) != 1 {
		t.Fatalf("replay reuses the recorded read, got %d fetches", len(p.calls))
	}
	if e.embeds != 2 || e.texts[1] != e.texts[0] {
		t.Fatalf("replay recomputes the embed from the recorded text: %d embeds, %q", e.embeds, e.texts)
	}
	again, _ := e.Embed([]string{e.texts[1]})
	for i, f := range again[0] {
		if vec[i] != f {
			t.Fatalf("recomputed vector differs at %d: %v vs %v", i, vec[i], f)
		}
	}
	if n := len(h.rows("incidents")); n != 1 {
		t.Fatalf("replay duplicated the write: %d rows", n)
	}
	if n := len(h.rows("loop_notices")); n != 1 {
		t.Fatalf("replay duplicated the notice: %d", n)
	}
	if h.asker.calls != 0 {
		t.Fatalf("replay called silclm %d times", h.asker.calls)
	}
	if n := h.eventCount(runID, "embed"); n != 0 {
		t.Fatalf("replay recorded %d embed events", n)
	}
}

func TestLoopEmbedStepFailsTheItemWithoutTheEmbedWorker(t *testing.T) {
	url := "https://example.test/silc/CHANGELOG"
	h := newBareHarness(t, "2026-10-02T15:00:00Z", "loop_embed_plan.json")
	h.k.fetch = (&fakePages{html: map[string]string{url: changelogHTML}}).fetch
	h.k.embedder = nil
	h.seed("dependencies", "silc", map[string]any{"name": "silc", "changelog": url})
	h.runNow("ReleaseIndex", "req-1")
	run := h.runsOf("ReleaseIndex")[0]
	if run["outcome"] == "succeeded" {
		t.Fatalf("an embed without the worker must not succeed: %v", run)
	}
	if n := len(h.rows("incidents")); n != 0 {
		t.Fatalf("no row may be written without its vector, got %d", n)
	}
}
