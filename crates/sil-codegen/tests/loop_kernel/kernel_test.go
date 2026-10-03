package main

// Replay traces for the loop kernel against the lowered examples/rfiChaseApp
// (plan.json) and examples/oneThingApp (onething_plan.json) plans. The Rust
// test `loop_kernel_replay_traces` copies this file, the kernel template, and
// both plans into a temp module and runs `go test`.

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
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
	h.asker.replies = []string{"Sure! Here is a reminder.", `{"subject":"only a subject"}`, `{"subject":"","body":"x"}`}
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

// ---------------------------------------------------------------- oneThingApp

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

const goodBrief = `{"facts":"EverydayMeal SOW signed by the client on 2026-10-01.","goals":"Execute both EverydayMeal SOWs this week.","pressure":"Neither SOW is fully executed."}`
const goodAction = `{"sentence":"Send both EverydayMeal SOWs through Sign.com today."}`

func newOneThing(t *testing.T, at string) (*harness, *fakeMcp) {
	t.Helper()
	h := newBareHarness(t, at, "onething_plan.json")
	h.asker.replies = []string{goodBrief, goodAction}
	m := &fakeMcp{}
	h.k.mcp = m.call
	t.Setenv("MOZ_MCP_TOKEN", "moz-secret")
	return h, m
}

func (h *harness) runNow(loop, id string) {
	h.t.Helper()
	h.seed("loop_requests", id, map[string]any{"loop": loop, "requested_by": "Dan Stephenson", "status": "pending", "run_id": ""})
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

func TestOneThingReadsMozOverMcpAndWritesEachRun(t *testing.T) {
	h, m := newOneThing(t, "2026-10-02T15:00:00Z")
	rec := &recordingAsker{fakeAsker: fakeAsker{replies: []string{goodBrief, goodAction}}}
	h.k.asker = rec
	h.runNow("OneThingToday", "req-1")

	run := h.runsOf("OneThingToday")[0]
	if run["outcome"] != "succeeded" {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 4 {
		t.Fatalf("four Moz streams, got %d calls", len(m.calls))
	}
	first := m.calls[0]
	if first["tool"] != "kb_jsonl_read_window" || first["token"] != "moz-secret" ||
		!strings.HasSuffix(first["server"].(string), "/mcp") {
		t.Fatalf("call: %v", first)
	}
	args := first["args"].(map[string]any)
	if args["path"] != "memory/decisions.jsonl" || args["tailRecords"] != float64(6) {
		t.Fatalf("args are sent as written in the contract: %v", args)
	}
	if !strings.Contains(rec.prompts[0], "Today is 2026-10-02 (Friday)") ||
		!strings.Contains(rec.prompts[0], "2026-09-26 through 2026-10-02") ||
		!strings.Contains(rec.prompts[0], "through 2026-10-08") {
		t.Fatalf("calendar window in the prompt: %s", rec.prompts[0])
	}
	if strings.Contains(rec.contexts[0], `"raw"`) || !strings.Contains(rec.contexts[0], "recent memory/decisions.jsonl") {
		t.Fatalf(":select keeps only parsed records: %s", rec.contexts[0])
	}
	action := h.one("daily_actions")
	if action["day"] != "2026-10-02" || action["at"] != "2026-10-02T15:00:00Z" ||
		action["sentence"] != "Send both EverydayMeal SOWs through Sign.com today." {
		t.Fatalf("daily action: %v", action)
	}
	notice := h.one("loop_notices")
	if notice["to"] != "Dan Stephenson" || notice["text"] != action["sentence"] {
		t.Fatalf("notice: %v", notice)
	}

	// A later run the same day records its own action alongside the first.
	rec.replies = []string{goodBrief, `{"sentence":"Something else entirely."}`}
	rec.calls = 0
	h.setClock("2026-10-02T16:00:00Z")
	h.runNow("OneThingToday", "req-2")
	if n := len(h.runsOf("OneThingToday")); n != 2 {
		t.Fatalf("want two runs, got %d", n)
	}
	actions := h.rows("daily_actions")
	if len(actions) != 2 {
		t.Fatalf("each run keeps its own action, got %d", len(actions))
	}
	seen := map[string]string{}
	for _, a := range actions {
		seen[a["at"].(string)] = a["sentence"].(string)
	}
	if seen["2026-10-02T16:00:00Z"] != "Something else entirely." || seen["2026-10-02T15:00:00Z"] != action["sentence"] {
		t.Fatalf("actions by run time: %v", seen)
	}
	if n := len(h.rows("loop_notices")); n != 2 {
		t.Fatalf("each run posts its own notice, got %d", n)
	}
}

func TestOneThingScheduleFiresAtFiveUtc(t *testing.T) {
	h, m := newOneThing(t, "2026-10-03T04:59:00Z")
	h.tick()
	if n := len(h.runsOf("OneThingToday")); n != 0 {
		t.Fatalf("too early, got %d runs", n)
	}
	h.setClock("2026-10-03T05:00:10Z")
	h.tick()
	runs := h.runsOf("OneThingToday")
	if len(runs) != 1 || runs[0]["trigger"] != "schedule" || runs[0]["outcome"] != "succeeded" {
		t.Fatalf("05:00 UTC run: %v", runs)
	}
	if len(m.calls) != 4 || h.one("daily_actions")["day"] != "2026-10-03" {
		t.Fatalf("calls %d, action %v", len(m.calls), h.rows("daily_actions"))
	}
}

func TestOneThingFailsClosedWithoutToken(t *testing.T) {
	h, m := newOneThing(t, "2026-10-02T15:00:00Z")
	t.Setenv("MOZ_MCP_TOKEN", "")
	h.runNow("OneThingToday", "req-1")
	run := h.runsOf("OneThingToday")[0]
	if run["outcome"] != "failed" || !strings.Contains(run["detail"].(string), "MOZ_MCP_TOKEN is not set") {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 0 || h.asker.calls != 0 {
		t.Fatalf("no network or model calls without a token: mcp %d, ask %d", len(m.calls), h.asker.calls)
	}
}

func TestOneThingRetriesThenFailsWhenMozIsDown(t *testing.T) {
	h, m := newOneThing(t, "2026-10-02T15:00:00Z")
	m.fail = true
	h.runNow("OneThingToday", "req-1")
	run := h.runsOf("OneThingToday")[0]
	if run["outcome"] != "failed" || !strings.Contains(run["detail"].(string), "HTTP 503") {
		t.Fatalf("run: %v", run)
	}
	if len(m.calls) != 3 {
		t.Fatalf("one try plus two retries, got %d", len(m.calls))
	}
	if n := len(h.rows("daily_actions")); n != 0 {
		t.Fatalf("nothing written, got %d", n)
	}
	// Replay reuses the recorded failure instead of calling again.
	if err := h.k.ResumeRun(run["id"].(string)); err != nil {
		t.Fatal(err)
	}
	if len(m.calls) != 3 {
		t.Fatalf("replay must not call Moz again, got %d", len(m.calls))
	}
}

func TestOneThingRejectsBlankBrief(t *testing.T) {
	h, _ := newOneThing(t, "2026-10-02T15:00:00Z")
	h.asker.replies = []string{`{"facts":"x","goals":"y","pressure":" "}`, goodAction}
	h.runNow("OneThingToday", "req-1")
	run := h.runsOf("OneThingToday")[0]
	if run["outcome"] != "failed" {
		t.Fatalf("blank pressure is not a valid Brief: %v", run)
	}
	if n := len(h.rows("daily_actions")); n != 0 {
		t.Fatalf("nothing written, got %d", n)
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

func TestCheckContractTakesFirstObjectAndRejectsBlanks(t *testing.T) {
	fields := []FieldSpec{{Name: "first", Type: "Str"}, {Name: "fourth", Type: "Str"}}
	reply := "Sure.\n{\"first\":\"Send the SOW\",\"fourth\":\"File the W-8\"}\nJustification: the {pressure} is real."
	got, err := checkContract(fields, reply)
	if err != nil {
		t.Fatal(err)
	}
	if got["first"] != "Send the SOW" || got["fourth"] != "File the W-8" {
		t.Fatalf("got %v", got)
	}
	blank := "{\"first\":\"Send the SOW\",\"fourth\":\"\"}"
	if _, err := checkContract(fields, blank); err == nil || !strings.Contains(err.Error(), "fourth") {
		t.Fatalf("blank Str should fail, got %v", err)
	}
	if _, err := checkContract(fields, "no object here"); err == nil {
		t.Fatal("expected a missing-object error")
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
