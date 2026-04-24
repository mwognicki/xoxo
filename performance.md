1. Stop load_chat on every tick (main.rs:165). Only call it in reaction to events that actually need a fresh chat snapshot (or ideally never — sync_chat_summary should work from the  
   event payload). This alone may fix most of the lag for established chats. Quick win: move it inside the match ... Some(event) => arm AND gate it on event kinds that mutate the stored
   chat.
2. Cache the rendered conversation lines on App. Keep ConversationLines (or its Vec<Line<'static>>) in a field, invalidated on history mutation or started_at ticks when the spinner
   phase changes. The render path then clones / borrows the cached vector. Input keystrokes, which don't touch history, become O(1) frame cost.
3. Fix the O(n²) tool-call matching. Build a HashMap<ChatToolCallId, &ToolCallEvent> once per rebuild (in build_conversation_lines) and pass it through. Turns has_matching_tool_start
   + tool_outcome into O(1) lookups. Cheaper than fix #2 but only helps tool-heavy chats.
4. Ratchet down repaint frequency. terminal().draw(...) runs every loop iteration (every 200ms even when idle, immediately after every keypress). The doing-indicator and tool dot use
   started_at.elapsed() to animate, so some idle redraws are needed — but not at max rate. Only redraw when: input changed, a bus event fired, or the animation phase (every 200ms)       
   crossed a boundary.