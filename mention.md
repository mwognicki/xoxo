Today the TUI's bottom input box (crates/lib/xoxo-tui/src/ui.rs:156-185, backed by App::input: String) accepts free-form text. There is no way to reference a workspace file or
directory by path without the user typing it from memory. This change introduces a fzf-style popup: typing @ in the input opens a floating list anchored to the input; characters
typed after @ filter the list live (case-insensitive substring); Tab confirms the highlighted entry by replacing the @… fragment with @<path-relative-to-workspace-root> (prefix
retained). Both files and directories are selectable. Directory semantics downstream (expanding to all files under it) are out of scope for this change — we only produce the
@<path> token in the input buffer.

Spec locked with the user:
- Source: workspace root walked with the ignore crate (honors .gitignore, hides .git/, target/, etc. via standard_filters(true)).
- Filter: case-insensitive plain substring (no fuzzy ranking).
- Display cap: 5 entries at a time.
- Confirm: Tab inserts @<rel-path>, replacing the @… fragment in input.
- Dismissal: Esc, typing a whitespace character, or deleting past the @ closes the popup. (Popup closes; the typed text remains in input.)

Approach decision locked: a new dedicated MentionPopup state on App, independent of the existing Modal (which centers, swallows keys, and doesn't anchor). The input keeps handling
Char/Backspace/Paste as usual; after each edit, the popup observes input and recomputes its filtered results.

Critical files

- crates/lib/xoxo-tui/src/app.rs — add mention_popup: Option<MentionPopup> and workspace_root: PathBuf fields to App; initialize both in new_with_storage.
- crates/lib/xoxo-tui/src/app/mention.rs — new module owning MentionPopup, MentionEntry, filtering, and workspace walk.
- crates/lib/xoxo-tui/src/app/events.rs — route keys to the popup when open, trigger popup open/update on input edits, handle Tab/Esc/Up/Down.
- crates/lib/xoxo-tui/src/ui.rs — pass the input Rect into a new render_mention_popup call after render_input.
- crates/lib/xoxo-tui/src/ui/mention.rs — new module that renders the popup anchored above (or below, if no room) the input.
- crates/lib/xoxo-tui/src/ui.rs + src/lib.rs — wire the new ui::mention submodule and app::mention re-exports.
- crates/lib/xoxo-tui/Cargo.toml — add ignore = "0.4" (already used in crates/agents/nerd/Cargo.toml:24 and crates/agents/nerd-ast/Cargo.toml:12).

Design

New types (app/mention.rs)

pub struct MentionPopup {
 /// Byte index in `App::input` where the trigger `@` sits.
 pub trigger_at: usize,
 /// Lowercased filter (chars after `@`, up to the cursor/end).
 filter: String,
 /// Pre-walked snapshot of candidate entries (paths relative to workspace root).
 all_entries: Arc<[MentionEntry]>,
 /// Indices into `all_entries` matching the current filter, truncated to MAX_VISIBLE.
 visible: Vec<usize>,
 /// Index into `visible` of the currently highlighted row.
 pub selected: usize,
}

pub struct MentionEntry {
 pub rel_path: String, // forward-slash, workspace-relative
 pub is_dir: bool,
}

pub const MAX_VISIBLE: usize = 5;

Walk once per popup-open (not per keystroke). The workspace is typically small enough that a single eager walk using ignore::WalkBuilder::new(&root).standard_filters(true)
(mirroring crates/agents/nerd/src/tools/find_files.rs:66) is simpler than incremental indexing; cache it on the popup itself (Arc<[MentionEntry]>). The popup is short-lived —
dropped on Esc/space/confirm — so the cache lifetime is naturally bounded to one user interaction.

If perf ever bites, the cache can be promoted onto App with an invalidation hook, but that is out of scope here.

Methods

- MentionPopup::open(workspace_root: &Path, trigger_at: usize) -> Result<Self> — walks workspace, builds all_entries, sets filter = "", visible = (0..min(len, 5)).collect().
- set_filter(&mut self, filter: &str) — lowercases, rebuilds visible with substring match, truncates to 5, clamps selected.
- select_prev(&mut self) / select_next(&mut self) — bounded by visible.len().
- selected_entry(&self) -> Option<&MentionEntry>.

Event flow (app/events.rs)

Order within handle_event → Event::Key (insert after modal handling at line 100, before Ctrl+S at line 102):

1. Popup is open — intercept keys first:
- Tab → commit: replace input[trigger_at..] with @<rel_path>, then mention_popup = None.
- Esc → close (clear popup, leave input untouched).
- Up/Down → select_prev / select_next. No page keys (only 5 visible).
- Backspace → fall through to the normal handler, then re-check filter. If cursor passes the @, close popup.
- Char(c) where c.is_whitespace() → close popup, then fall through so the whitespace still lands in input.
- Other printable Char(c) → fall through to normal handler, then recompute filter.
2. Popup is closed — after the existing KeyCode::Char(c) branch at line 131 appends to input, check c == '@'; if so, call self.open_mention_popup().
3. After Backspace (line 134), if popup is open and input.len() <= trigger_at, close the popup (the @ itself was deleted).
4. After any Char/Backspace that doesn't close the popup, call self.refresh_mention_filter() to slice input[trigger_at + 1..] into the popup.

Event::Paste at line 62 also needs handling: if the pasted content contains @, do nothing special (the popup is only opened by a single keystroke, not by pastes — keeps semantics
simple, and pastes of large chunks with @ in the middle shouldn't surprise-trigger a picker).

Event::Key(KeyCode::Enter) at line 136 — if the popup is open, Enter does not submit; instead it behaves like Tab (commit selection). Rationale: once the user sees a picker they're
in picking mode, and accidental Enter shouldn't fire off the message with a half-typed @fragment.

Mutating input on commit

input is a plain String (confirmed at crates/lib/xoxo-tui/src/app.rs:32), and there is no separate cursor — character insertion is always at the end (app/events.rs:131). So
committing is self.input.truncate(trigger_at); self.input.push('@'); self.input.push_str(&rel_path);. No cursor math needed.

Rendering (ui/mention.rs)

Signature: pub fn render_mention_popup(f: &mut Frame, input_area: Rect, popup: &MentionPopup).

- Height: visible.len().max(1) + 2 (one row per entry, + 2 for borders). Cap at 7 total (5 entries + borders).
- Width: min(input_area.width, max_entry_width + 4), min 24.
- X: input_area.x (left-aligned with the input).
- Y: prefer above — input_area.y.saturating_sub(height). If that would collide with the conversation (y < some small threshold, e.g. 3), fall back to below: input_area.y +
input_area.height (but the input is already at the bottom of draw_main chunks[1]; effectively we always render above unless the terminal is pathologically short, in which case we
fall back to centered).
- Render: Clear + Block::bordered().title(" Files (Tab to pick, Esc to close) "), then a simple list where the selected row uses a reversed/highlighted style. Directories annotated
with trailing / in their label.
- Empty results: show "(no matches)" body.

Called from ui.rs::draw_main after render_input so it paints on top. The Rect for the input is already available as chunks[1] (line 65).

Workspace root

Currently derived at render-time from std::env::current_dir() (ui.rs:82, ui.rs:293). Add pub workspace_root: PathBuf on App, set in new_with_storage via
std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")), and reuse it both for the popup walk and for the existing header/status display (small cleanup — the two
env::current_dir() calls become field reads, fewer syscalls per frame).

Cargo

Add to crates/lib/xoxo-tui/Cargo.toml under [dependencies]:
ignore = "0.4"

No feature gating — the popup is a core TUI feature.

Tests

Unit tests in app/mention.rs:
- filter_substring_is_case_insensitive — entries ["SRC/lib.rs", "src/app.rs"], filter "src" matches both.
- filter_truncates_to_five — with 20 matching entries, visible.len() == 5.
- select_next_bounded / select_prev_bounded — clamps at 0 and visible.len()-1.
- empty_filter_shows_first_five — filter "" returns top 5 in walk order.
- selected_clamps_on_filter_narrowing — selected=3, filter narrows visible to 2 → selected becomes 1.

Unit tests in app/events.rs (extend existing module):
- at_sign_opens_mention_popup — press @, popup is Some, trigger_at == 0.
- space_after_at_closes_popup — press @, press ' ', popup is None, input ends with "@ ".
- backspace_past_at_closes_popup — press @, press x, backspace, backspace, popup is None.
- tab_commits_selection — seed popup with a known entry list, press Tab, assert input == "@<path>" and popup is None.
- esc_closes_popup_leaves_input — press @, press x, press Esc, input is "@x", popup is None.
- enter_while_popup_open_commits_not_submits — press @, press Enter, assert pending_submission.is_none() and input has committed path.
- typing_filter_updates_popup — press @, press a, assert popup.filter == "a".

For the workspace-walk tests, use tempfile::tempdir() (already a dev-dep at xoxo-tui/Cargo.toml:28) to build a fake tree with a .gitignore, instantiate
MentionPopup::open(tempdir_path, 0), and assert ignored files are absent.

Verification

1. cargo check -p xoxo-tui — crate compiles.
2. cargo check --workspace --all-features and cargo check --workspace --no-default-features — feature matrix unaffected.
3. cargo test -p xoxo-tui --all-features — unit tests pass.
4. cargo clippy --workspace --all-features -- -D warnings — no warnings (repo-wide invariant: #![deny(warnings)] per CLAUDE.md).
5. cargo fmt.
6. Manual smoke test:
- cargo run -p xoxo --features tui -- tui
- In the TUI input, type @ → popup appears above input listing up to 5 workspace entries (verify target/ and hidden dirs absent).
- Type cargo → list narrows to Cargo.lock, Cargo.toml, etc.
- Press Down twice, Tab → input now contains @<selected-path>.
- Type @s, press Esc → input is "@<path>@s", popup gone.
- Type /help → works unchanged; the @ handling does not interfere with slash commands (they never contain @).
7. Resize the terminal very short (< 8 rows) to confirm the popup fallback doesn't panic.

Risks / notes

- Walk cost on open. A full ignore::Walk on a huge repo can be slow (tens of ms). For this repo it's fine; if it becomes a problem, hoist the cache onto App and invalidate on a
coarse timer or on explicit refresh. Not doing that now.
- Unicode in input. input.truncate(trigger_at) uses byte indices; trigger_at must be a char boundary. Since @ is ASCII and we set trigger_at = input.len() at the moment of pressing
@ (before any post-@ chars are appended), it's always on a boundary. Add a debug assert for safety.
- Multi-line paste with @. Explicitly not triggering popup on paste (see event flow §Event::Paste). Documented in code comments.
- Interaction with the existing modal. If a modal is already open, @ is swallowed by the modal branch at app/events.rs:88-99 and the popup never opens — which is the correct
behavior.
