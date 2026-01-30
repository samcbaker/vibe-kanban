#!/usr/bin/env python3
"""
Ralph Loop - Enriched UI for Autonomous AI Development Loop
Reference: https://github.com/ghuntley/how-to-ralph-wiggum

This script provides a rich terminal UI with split panels for viewing
Claude/Codex output in real-time.
"""

import argparse
import atexit
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

try:
    from rich.console import Console, Group
    from rich.layout import Layout
    from rich.live import Live
    from rich.panel import Panel
    from rich.table import Table
    from rich.text import Text
except ImportError:
    print("Error: 'rich' library is required.")
    print("Install it with: pip install rich")
    sys.exit(1)


# ============================================================================
# Terminal Customization
# ============================================================================

# Global console for terminal title (created early, before Rich Live takes over)
_title_console: Optional[Console] = None


def set_terminal_title(title: str) -> None:
    """Set the terminal tab/window title."""
    global _title_console

    # Method 1: Use Rich's built-in method (best compatibility)
    if _title_console is None:
        _title_console = Console(stderr=True)  # Use stderr to avoid Rich Live interference
    _title_console.set_window_title(title)

    # Method 2: Also try direct escape sequence via stderr (fallback for some terminals)
    # stderr is not captured by Rich's Live display
    sys.stderr.write(f"\033]0;{title}\007")
    sys.stderr.flush()


def set_iterm2_tab_color(r: int, g: int, b: int) -> None:
    """Set iTerm2 tab color (ignored by other terminals)."""
    if os.environ.get("TERM_PROGRAM") == "iTerm.app":
        sys.stderr.write(f"\033]6;1;bg;red;brightness;{r}\007")
        sys.stderr.write(f"\033]6;1;bg;green;brightness;{g}\007")
        sys.stderr.write(f"\033]6;1;bg;blue;brightness;{b}\007")
        sys.stderr.flush()


def reset_terminal() -> None:
    """Reset terminal title and tab color on exit."""
    global _title_console
    if _title_console:
        _title_console.set_window_title("")
    sys.stderr.write("\033]0;\007")
    if os.environ.get("TERM_PROGRAM") == "iTerm.app":
        sys.stderr.write("\033]6;1;bg;*;default\007")
    sys.stderr.flush()


def setup_terminal(engine: str, mode: str) -> None:
    """Configure terminal appearance for the session."""
    title = f"🚀 Ralph Loop - {engine.capitalize()} {mode.capitalize()}"
    set_terminal_title(title)

    # Purple/violet theme for iTerm2 (RGB: 138, 43, 226)
    set_iterm2_tab_color(138, 43, 226)

    # Reset on exit
    atexit.register(reset_terminal)


@dataclass
class LogEntry:
    """A single log entry in the log panel."""
    timestamp: datetime
    icon: str
    color: str
    title: str
    content: str = ""

    def to_rich_text(self, width: int = 60) -> Text:
        """Convert to rich Text for display."""
        time_str = self.timestamp.strftime("%H:%M:%S")
        text = Text()
        text.append(f"[{time_str}] ", style="dim")
        text.append(f"{self.icon} ", style=self.color)
        text.append(self.title, style=self.color)
        if self.content:
            # Truncate content for display
            content_preview = self.content[:200]
            if len(self.content) > 200:
                content_preview += "..."
            text.append(f"\n           {content_preview}", style="dim")
        return text


def format_duration(total_seconds: int) -> str:
    """Format seconds into a human-readable duration string."""
    if total_seconds < 60:
        return f"{total_seconds}s"
    elif total_seconds < 3600:
        minutes = total_seconds // 60
        seconds = total_seconds % 60
        return f"{minutes}m {seconds}s"
    else:
        hours = total_seconds // 3600
        minutes = (total_seconds % 3600) // 60
        seconds = total_seconds % 60
        return f"{hours}h {minutes}m {seconds}s"


@dataclass
class LoopState:
    """State for the current loop session."""
    engine: str
    mode: str
    iteration: int
    max_iterations: int
    prompt_file: str
    script_dir: str  # .ralph/ directory (where prompts live)
    project_dir: str = ""  # Project root (parent of .ralph/)
    current_tool: Optional[str] = None
    tool_name_by_id: dict = field(default_factory=dict)
    cursor_model: Optional[str] = None
    input_tokens: int = 0
    output_tokens: int = 0
    start_time: datetime = field(default_factory=datetime.now)
    iteration_start_time: Optional[datetime] = None
    last_iteration_duration: Optional[int] = None  # Duration in seconds
    log_entries: list = field(default_factory=list)
    session_id: Optional[str] = None
    model: Optional[str] = None
    is_running: bool = False
    last_error: Optional[str] = None
    debug_mode: bool = False
    debug_log_path: Optional[str] = None

    def add_log(self, icon: str, color: str, title: str, content: str = "") -> None:
        """Add a log entry."""
        self.log_entries.append(LogEntry(
            timestamp=datetime.now(),
            icon=icon,
            color=color,
            title=title,
            content=content
        ))

        if self.debug_mode and self.debug_log_path:
            try:
                with open(self.debug_log_path, "a", encoding="utf-8") as f:
                    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                    f.write(f"[{timestamp}] {icon} {title}\n")
                    if content:
                        f.write(f"{content}\n")
                    f.write("-" * 40 + "\n")
            except Exception:
                pass

    def log_raw_output(self, line: str) -> None:
        """Log raw output line to debug log."""
        if self.debug_mode and self.debug_log_path:
            try:
                with open(self.debug_log_path, "a", encoding="utf-8") as f:
                    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                    f.write(f"[{timestamp}] [RAW] {line.strip()}\n")
            except Exception:
                pass

        if self.debug_mode and self.debug_log_path:
            try:
                with open(self.debug_log_path, "a", encoding="utf-8") as f:
                    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                    f.write(f"[{timestamp}] {icon} {title}\n")
                    if content:
                        f.write(f"{content}\n")
                    f.write("-" * 40 + "\n")
            except Exception:
                pass

    def get_iteration_duration(self) -> str:
        """Get formatted duration for current iteration."""
        if self.iteration_start_time:
            delta = datetime.now() - self.iteration_start_time
            return format_duration(int(delta.total_seconds()))
        return "-"

    def get_total_duration(self) -> str:
        """Get formatted total duration since start."""
        delta = datetime.now() - self.start_time
        return format_duration(int(delta.total_seconds()))

    def get_last_iteration_duration(self) -> str:
        """Get formatted duration of last completed iteration."""
        if self.last_iteration_duration is not None:
            return format_duration(self.last_iteration_duration)
        return "-"


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments (same interface as loop.sh)."""
    parser = argparse.ArgumentParser(
        description="Ralph Loop - Enriched UI for Autonomous AI Development",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ./enriched_loop.sh              # Build mode with Claude (default)
  ./enriched_loop.sh --codex      # Build mode with Codex
  ./enriched_loop.sh --cursor     # Build mode with Cursor Agent
  ./enriched_loop.sh --cursor-codex        # Cursor Agent with Codex model
  ./enriched_loop.sh --cursor-grok         # Cursor Agent with Grok model
  ./enriched_loop.sh --cursor-claude-sonnet # Cursor Agent with Claude Sonnet
  ./enriched_loop.sh --cursor-claude-opus  # Cursor Agent with Claude Opus
  ./enriched_loop.sh --cursor-gemini       # Cursor Agent with Gemini model
  ./enriched_loop.sh plan         # Plan mode with Claude
  ./enriched_loop.sh 20           # Build mode with Claude, max 20 iterations
  ./enriched_loop.sh plan 5       # Plan mode with Claude, max 5 iterations
        """
    )
    engine_group = parser.add_mutually_exclusive_group()
    engine_group.add_argument(
        "--codex",
        action="store_true",
        help="Use Codex engine instead of Claude"
    )
    engine_group.add_argument(
        "--cursor",
        "--cursor-agent",
        dest="cursor_agent",
        action="store_true",
        help="Use Cursor Agent engine instead of Claude"
    )
    engine_group.add_argument(
        "--cursor-codex",
        dest="cursor_variant",
        action="store_const",
        const="gpt-5.2-codex",
        help="Use Cursor Agent with Codex model"
    )
    engine_group.add_argument(
        "--cursor-grok",
        dest="cursor_variant",
        action="store_const",
        const="grok",
        help="Use Cursor Agent with Grok model"
    )
    engine_group.add_argument(
        "--cursor-claude-sonnet",
        dest="cursor_variant",
        action="store_const",
        const="sonnet-4.5",
        help="Use Cursor Agent with Claude Sonnet model"
    )
    engine_group.add_argument(
        "--cursor-claude-opus",
        dest="cursor_variant",
        action="store_const",
        const="opus-4.5",
        help="Use Cursor Agent with Claude Opus model"
    )
    engine_group.add_argument(
        "--cursor-gemini",
        dest="cursor_variant",
        action="store_const",
        const="gemini-3-pro",
        help="Use Cursor Agent with Gemini model"
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging to .ralph/session_log.log"
    )
    parser.add_argument("mode_or_iterations", nargs="?", default=None,
                        help="'plan' for plan mode, or a number for max iterations")
    parser.add_argument("iterations", nargs="?", type=int, default=None,
                        help="Max iterations (when using plan mode)")

    args = parser.parse_args()

    # Parse mode and iterations similar to the shell script
    cursor_model = None
    if args.codex:
        engine = "codex"
    elif args.cursor_agent:
        engine = "cursor-agent"
    elif args.cursor_variant:
        engine = "cursor-agent"
        cursor_model = args.cursor_variant
    else:
        engine = "claude"

    if args.mode_or_iterations == "plan":
        mode = "plan"
        prompt_file = "PROMPT_plan.md"
        max_iterations = args.iterations if args.iterations else 0
    elif args.mode_or_iterations and args.mode_or_iterations.isdigit():
        mode = "build"
        prompt_file = "PROMPT_build.md"
        max_iterations = int(args.mode_or_iterations)
    else:
        mode = "build"
        prompt_file = "PROMPT_build.md"
        max_iterations = 0

    return argparse.Namespace(
        engine=engine,
        mode=mode,
        prompt_file=prompt_file,
        max_iterations=max_iterations,
        debug=args.debug,
        cursor_model=cursor_model
    )


def parse_json_event(line: str) -> Optional[dict]:
    """Parse a JSON event from a line of output."""
    line = line.strip()
    if not line:
        return None
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return None


def handle_event(event: dict, state: LoopState) -> None:
    """Handle a parsed JSON event and update state."""
    event_type = event.get("type", "")

    # Handle init event
    if event_type == "init" or "session_id" in event:
        state.session_id = event.get("session_id", event.get("sessionId"))
        state.model = event.get("model")
        if state.model:
            state.add_log("🤖", "cyan", f"Model: {state.model}")

    # Handle system events
    if event_type == "system":
        message = event.get("message", event.get("subtype", ""))
        if message:
            state.add_log("ℹ️", "blue", f"System: {message}")

    # Handle Gemini message events
    if event_type == "message":
        role = event.get("role")
        content = event.get("content", "")
        if isinstance(content, list):
            content = "".join(
                item.get("text", "") for item in content if isinstance(item, dict)
            )
        if role == "assistant" and content and len(content) > 10:
            state.add_log("💬", "white", "Assistant", content[:300])

    # Handle assistant message events
    if event_type in ("assistant", "assistant.message", "content_block_delta"):
        content = ""
        content_list = None

        # Check for content in message field (Claude's actual format: event["message"]["content"])
        message = event.get("message", {})
        if "content" in message:
            content_list = message.get("content", [])
        elif "content" in event:
            content_list = event.get("content", [])
        elif "delta" in event:
            delta = event.get("delta", {})
            if delta.get("type") == "text_delta":
                content = delta.get("text", "")

        # Process content_list if we found one
        if content_list is not None:
            if isinstance(content_list, list):
                for item in content_list:
                    if isinstance(item, dict):
                        if item.get("type") == "text":
                            content += item.get("text", "")
                        elif item.get("type") == "tool_use":
                            tool_name = item.get("name", "unknown")
                            tool_input = item.get("input", {})
                            state.current_tool = tool_name
                            input_preview = str(tool_input)[:100]
                            state.add_log("🔧", "yellow", f"Tool: {tool_name}", input_preview)
            elif isinstance(content_list, str):
                content = content_list

        if content and len(content) > 10:
            state.add_log("💬", "white", "Assistant", content[:300])

    # Handle tool use events directly
    if event_type == "tool_use":
        tool_name = event.get("name") or event.get("tool") or event.get("tool_name") or "unknown"
        tool_input = event.get("input", event.get("arguments", event.get("parameters", {})))
        tool_id = event.get("tool_id", event.get("id", event.get("call_id")))
        if tool_id and tool_name:
            state.tool_name_by_id[tool_id] = tool_name
        state.current_tool = tool_name
        input_preview = str(tool_input)[:150] if tool_input else ""
        state.add_log("🔧", "yellow", f"Tool: {tool_name}", input_preview)

    # Handle content block start (tool use)
    if event_type == "content_block_start":
        content_block = event.get("content_block", {})
        if content_block.get("type") == "tool_use":
            tool_name = content_block.get("name", "unknown")
            state.current_tool = tool_name
            state.add_log("🔧", "yellow", f"Tool: {tool_name}")

    # Handle tool result events
    if event_type == "tool_result":
        tool_id = event.get("tool_id", event.get("id", event.get("call_id")))
        tool_name = (
            event.get("name")
            or state.tool_name_by_id.get(tool_id)
            or state.current_tool
            or "unknown"
        )
        output = event.get("output", event.get("content", event.get("result", "")))
        error_info = event.get("error")
        status = event.get("status")
        is_error = bool(error_info) or event.get("is_error", False) or status == "error"

        if is_error:
            if isinstance(error_info, dict):
                output = error_info.get("message", output)
            state.add_log("⚠️", "red", f"Tool Error: {tool_name}", str(output)[:200])
        else:
            output_preview = str(output)[:150] if output else ""
            state.add_log("✓", "green", f"Tool Result: {tool_name}", output_preview)

        state.current_tool = None
        if tool_id:
            state.tool_name_by_id.pop(tool_id, None)

    # Handle usage/token events
    if "usage" in event:
        usage = event.get("usage", {})
        state.input_tokens = usage.get("input_tokens", state.input_tokens)
        state.output_tokens = usage.get("output_tokens", state.output_tokens)
    if "stats" in event:
        stats = event.get("stats", {})
        state.input_tokens = stats.get("input_tokens", state.input_tokens)
        state.output_tokens = stats.get("output_tokens", state.output_tokens)

    # Handle result events (iteration complete)
    if event_type == "result":
        # Log the final result text
        result_text = event.get("result", "")
        if result_text:
            state.add_log("📋", "white", "Result", result_text[:300])
        elif event.get("status"):
            state.add_log("📋", "white", "Result", str(event.get("status")))

        usage = event.get("usage", {})
        state.input_tokens = usage.get("input_tokens", state.input_tokens)
        state.output_tokens = usage.get("output_tokens", state.output_tokens)
        stats = event.get("stats", {})
        state.input_tokens = stats.get("input_tokens", state.input_tokens)
        state.output_tokens = stats.get("output_tokens", state.output_tokens)
        cost_usd = event.get("cost_usd", 0)
        if cost_usd:
            state.add_log("💰", "cyan", f"Cost: ${cost_usd:.4f}")

    # Handle error events
    if event_type == "error":
        error_msg = event.get("error", event.get("message", {}))
        if isinstance(error_msg, dict):
            error_text = error_msg.get("message", str(error_msg))
        else:
            error_text = str(error_msg)
        state.last_error = error_text
        state.add_log("❌", "red", "Error", error_text)

    # =========================================================================
    # CODEX EVENT HANDLERS
    # =========================================================================

    # Handle Codex thread.started
    if event_type == "thread.started":
        thread_id = event.get("thread_id", "")
        state.session_id = thread_id
        state.add_log("🧵", "cyan", f"Thread started", thread_id[:20] if thread_id else "")

    # Handle Codex turn.started
    if event_type == "turn.started":
        state.add_log("🔄", "blue", "Turn started")

    # Handle Codex turn.completed
    if event_type == "turn.completed":
        usage = event.get("usage", {})
        state.input_tokens = usage.get("input_tokens", state.input_tokens)
        cached = usage.get("cached_input_tokens", 0)
        state.output_tokens = usage.get("output_tokens", state.output_tokens)
        state.add_log("✅", "green", f"Turn completed", f"in:{state.input_tokens} out:{state.output_tokens} cached:{cached}")

    # Handle Codex turn.failed
    if event_type == "turn.failed":
        error_msg = event.get("error", event.get("message", "Unknown error"))
        state.last_error = str(error_msg)[:200]
        state.add_log("❌", "red", "Turn failed", str(error_msg)[:200])

    # Handle Codex item.started
    if event_type == "item.started":
        item = event.get("item", {})
        item_type = item.get("type", "unknown")
        item_id = item.get("id", "")
        # Try multiple field names for text content
        text = item.get("text") or item.get("content") or item.get("message") or item.get("summary") or ""

        if item_type == "command_execution":
            command = item.get("command", "")
            state.current_tool = "bash"
            state.add_log("⚡", "yellow", f"Command", command[:150])
        elif item_type == "agent_message":
            if text:
                state.add_log("💬", "white", "Agent", text[:300])
        elif item_type == "reasoning":
            if text:
                state.add_log("💭", "magenta", "Reasoning", text[:300])
        elif item_type == "file_change":
            path = item.get("path", item.get("file", ""))
            action = item.get("action", "modify")
            state.add_log("📝", "cyan", f"File {action}", path)
        elif item_type == "mcp_tool_call":
            tool_name = item.get("tool", item.get("name", "unknown"))
            state.current_tool = tool_name
            state.add_log("🔧", "yellow", f"MCP Tool: {tool_name}")
        elif item_type == "web_search":
            query = item.get("query", "")
            state.add_log("🔍", "blue", "Web search", query[:100])
        else:
            # Log any unrecognized item type to help debug
            preview = text[:100] if text else str(item)[:100]
            state.add_log("📦", "dim", f"Item: {item_type}", preview)

    # Handle Codex item.completed
    if event_type == "item.completed":
        item = event.get("item", {})
        item_type = item.get("type", "unknown")
        # Try multiple field names for text content
        text = item.get("text") or item.get("content") or item.get("message") or item.get("summary") or ""

        if item_type == "command_execution":
            exit_code = item.get("exit_code", item.get("exitCode", 0))
            output = item.get("output", item.get("stdout", ""))
            if exit_code != 0:
                state.add_log("⚠️", "red", f"Command failed (exit {exit_code})", str(output)[:150])
            else:
                state.add_log("✓", "green", "Command done", str(output)[:100] if output else "")
            state.current_tool = None
        elif item_type == "agent_message":
            if text:
                state.add_log("💬", "white", "Message", text[:300])
        elif item_type == "reasoning":
            if text:
                state.add_log("💭", "magenta", "Thought", text[:300])
        elif item_type == "file_change":
            path = item.get("path", item.get("file", ""))
            state.add_log("✓", "green", "File saved", path)
        elif item_type == "mcp_tool_call":
            result = item.get("result", item.get("output", ""))
            state.add_log("✓", "green", "MCP Result", str(result)[:150])
            state.current_tool = None
        else:
            # Log any unrecognized completed item to help debug
            if text:
                state.add_log("📦", "dim", f"Done: {item_type}", text[:150])

    # =========================================================================
    # CURSOR AGENT EVENT HANDLERS
    # =========================================================================

    if event_type == "tool_call":
        def _extract_cursor_tool_call(payload: dict) -> tuple[str, dict, object]:
            if not isinstance(payload, dict):
                return "unknown", {}, ""
            tool_name = (
                payload.get("name")
                or payload.get("tool_name")
                or payload.get("tool")
                or payload.get("type")
                or payload.get("function", {}).get("name")
            )
            tool_payload = payload
            if not tool_name and len(payload) == 1:
                tool_key = next(iter(payload.keys()))
                tool_payload = payload.get(tool_key, {}) if isinstance(payload, dict) else {}
                tool_name = tool_key
                for suffix in ("ToolCall", "toolCall", "Call"):
                    if tool_name.endswith(suffix):
                        tool_name = tool_name[: -len(suffix)]
                        break
            args = tool_payload.get("args", tool_payload.get("arguments", {}))
            result = tool_payload.get("result", tool_payload.get("output", ""))
            return tool_name or "unknown", args, result

        subtype = event.get("subtype", "")
        tool_call = event.get("tool_call", {}) or {}
        tool_name, tool_args, tool_result = _extract_cursor_tool_call(tool_call)
        tool_id = event.get("call_id", tool_call.get("id"))
        if tool_id and tool_name:
            state.tool_name_by_id[tool_id] = tool_name
        if subtype == "started":
            state.current_tool = tool_name
            input_preview = str(tool_args)[:150] if tool_args else ""
            state.add_log("🔧", "yellow", f"Tool: {tool_name}", input_preview)
        elif subtype == "completed":
            if isinstance(tool_result, dict):
                for key in ("success", "error", "failure"):
                    if key in tool_result:
                        tool_result = tool_result.get(key)
                        break
            output_preview = str(tool_result)[:150] if tool_result else ""
            state.add_log("✓", "green", f"Tool Result: {tool_name}", output_preview)
            state.current_tool = None
            if tool_id:
                state.tool_name_by_id.pop(tool_id, None)

    if event_type == "thinking":
        subtype = event.get("subtype", "")
        text = event.get("text", "")
        if subtype == "delta" and text and len(text) > 10:
            state.add_log("💭", "magenta", "Reasoning", text[:300])
        elif subtype == "completed":
            state.add_log("💭", "magenta", "Reasoning", "Completed")

    if event_type == "interaction_query":
        query_type = event.get("query_type", "")
        subtype = event.get("subtype", "")
        if query_type:
            state.add_log("ℹ️", "blue", "Interaction", f"{query_type} ({subtype})")


def build_info_panel(state: LoopState) -> Panel:
    """Build the left info panel."""
    table = Table.grid(padding=(0, 1))
    table.add_column(style="bold cyan", justify="right")
    table.add_column()

    table.add_row("Engine:", state.engine.upper())
    table.add_row("Mode:", state.mode)

    if state.max_iterations > 0:
        table.add_row("Iteration:", f"{state.iteration}/{state.max_iterations}")
    else:
        table.add_row("Iteration:", str(state.iteration))

    table.add_row("", "")  # Spacer

    if state.model:
        table.add_row("Model:", state.model)

    if state.current_tool:
        table.add_row("Tool:", Text(state.current_tool, style="yellow"))
    else:
        table.add_row("Tool:", Text("-", style="dim"))

    table.add_row("", "")  # Spacer

    table.add_row("Tokens:", "")
    table.add_row("  In:", f"{state.input_tokens:,}")
    table.add_row("  Out:", f"{state.output_tokens:,}")

    table.add_row("", "")  # Spacer

    # Duration section
    table.add_row("Duration:", "")
    table.add_row("  Total:", state.get_total_duration())
    table.add_row("  Iter:", state.get_iteration_duration())
    if state.last_iteration_duration is not None:
        table.add_row("  Last:", state.get_last_iteration_duration())

    table.add_row("", "")  # Spacer

    if state.is_running:
        status = Text("RUNNING", style="bold green")
    else:
        status = Text("IDLE", style="dim")
    table.add_row("Status:", status)

    if state.last_error:
        table.add_row("", "")
        table.add_row("Error:", Text(state.last_error[:30], style="red"))

    return Panel(
        table,
        title="[bold cyan]INFO[/bold cyan]",
        border_style="cyan",
        padding=(1, 1)
    )


def build_log_panel(state: LoopState, max_entries: int = 20) -> Panel:
    """Build the right log panel with scrollable entries."""
    if not state.log_entries:
        content = Text("Waiting for events...", style="dim italic")
    else:
        # Get last N entries
        entries = state.log_entries[-max_entries:]
        lines = []

        for entry in entries:
            lines.append(entry.to_rich_text())

        content = Group(*lines)

        # Add entry count indicator if there are more entries
        if len(state.log_entries) > max_entries:
            total = len(state.log_entries)
            showing_start = total - max_entries + 1
            indicator = Text(f"\n[Showing {showing_start}-{total} of {total} entries]", style="dim")
            content = Group(content, indicator)

    return Panel(
        content,
        title="[bold yellow]LOG[/bold yellow]",
        border_style="yellow",
        padding=(0, 1)
    )


def build_layout(state: LoopState) -> Layout:
    """Build the complete UI layout."""
    layout = Layout()

    # Create header
    header_text = Text()
    header_text.append("Ralph Loop - Enriched", style="bold white")
    header_text.append(" | ", style="dim")
    header_text.append(f"{state.engine.upper()}", style="cyan")
    header_text.append(" | ", style="dim")
    header_text.append(f"{state.mode}", style="green")

    header = Panel(header_text, style="bold", padding=(0, 1))

    # Split layout
    layout.split_column(
        Layout(header, name="header", size=3),
        Layout(name="body")
    )

    layout["body"].split_row(
        Layout(build_info_panel(state), name="info", size=22),
        Layout(build_log_panel(state), name="log"),
    )

    return layout


def get_prompt_content(prompt_file: str, script_dir: str) -> str:
    """Read the prompt file content."""
    prompt_path = Path(script_dir) / prompt_file
    if not prompt_path.exists():
        raise FileNotFoundError(f"Error: {prompt_file} not found in {script_dir}")
    return prompt_path.read_text()


def run_claude(prompt: str, state: LoopState) -> subprocess.Popen:
    """Spawn Claude CLI subprocess."""
    cmd = [
        "claude", "-p",
        "--dangerously-skip-permissions",
        "--output-format=stream-json",
        "--model", "opus",
        "--verbose",
    ]

    process = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,  # Capture stderr separately to see actual errors
        text=True,
        bufsize=1,
        cwd=state.project_dir  # Run from project root, not .ralph/
    )

    # Write prompt to stdin
    process.stdin.write(prompt)
    process.stdin.close()

    return process


def run_codex(prompt: str, state: LoopState) -> subprocess.Popen:
    """Spawn Codex CLI subprocess.

    Uses shell execution with proper escaping to handle large prompts
    with special characters, matching the behavior of loop.sh.
    """
    import shlex

    # Use shell execution to properly handle the prompt content
    # This matches the behavior of the original loop.sh:
    # codex exec "$(cat "$PROMPT_FILE")" --full-auto --sandbox danger-full-access --json
    escaped_prompt = shlex.quote(prompt)
    shell_cmd = f"codex exec {escaped_prompt} --full-auto --sandbox danger-full-access --skip-git-repo-check --json"

    process = subprocess.Popen(
        shell_cmd,
        shell=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,  # Capture stderr separately to see actual errors
        text=True,
        bufsize=1,
        cwd=state.project_dir  # Run from project root, not .ralph/
    )

    return process


def run_cursor_agent(prompt: str, state: LoopState) -> subprocess.Popen:
    """Spawn Cursor Agent CLI subprocess."""
    cmd = [
        "cursor-agent",
        "agent",
        "--print",
        "--output-format",
        "stream-json",
        "--force",
        "--approve-mcps",
        "--workspace",
        state.project_dir,
    ]

    if state.cursor_model:
        cmd.extend(["--model", state.cursor_model])

    if state.mode == "plan":
        cmd.extend(["--mode", "plan"])

    cmd.extend(["--", prompt])

    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
        cwd=state.project_dir
    )

    return process


def run_iteration(state: LoopState, prompt: str, live: Live) -> bool:
    """Run a single iteration of the loop. Returns True if successful."""
    state.iteration_start_time = datetime.now()
    state.is_running = True
    state.current_tool = None
    state.add_log("🚀", "cyan", f"Starting iteration {state.iteration}")

    live.update(build_layout(state))

    try:
        if state.engine == "codex":
            process = run_codex(prompt, state)
        elif state.engine == "cursor-agent":
            process = run_cursor_agent(prompt, state)
        else:
            process = run_claude(prompt, state)

        # Stream and parse output
        # Use iter() with readline for unbuffered real-time output
        # This bypasses Python's read-ahead buffering that blocks on "for line in file:"
        for line in iter(process.stdout.readline, ''):
            if state.debug_mode:
                state.log_raw_output(line)

            event = parse_json_event(line)
            if event:
                handle_event(event, state)
                live.update(build_layout(state))
            elif line.strip():
                # Log non-JSON output (errors, debug info) to help troubleshoot
                state.add_log("📝", "dim", "Output", line.strip()[:200])
                live.update(build_layout(state))

        # Wait for process to complete
        return_code = process.wait()

        # Calculate iteration duration
        if state.iteration_start_time:
            iteration_duration = int((datetime.now() - state.iteration_start_time).total_seconds())
            state.last_iteration_duration = iteration_duration
            duration_str = format_duration(iteration_duration)
        else:
            duration_str = "-"

        state.is_running = False
        state.current_tool = None

        if return_code == 0:
            state.add_log("✅", "green", f"Iteration {state.iteration} complete ({duration_str})")
        else:
            state.add_log("❌", "red", f"Iteration {state.iteration} failed ({duration_str}, exit code: {return_code})")
            # Read stderr for actual error message
            stderr_output = process.stderr.read() if process.stderr else ""
            if stderr_output:
                stderr_output = stderr_output.strip()
                state.last_error = stderr_output[:200]
                state.add_log("⚠️", "red", "Process Error", stderr_output[:500])

        live.update(build_layout(state))
        return return_code == 0

    except Exception as e:
        state.is_running = False
        state.last_error = str(e)
        state.add_log("💥", "red", "Exception", str(e))
        live.update(build_layout(state))
        return False


def main():
    """Main entry point."""
    args = parse_args()

    # Setup terminal title and colors
    setup_terminal(args.engine, args.mode)

    # Get directories
    script_dir = os.path.dirname(os.path.abspath(__file__))  # .ralph/
    project_dir = os.path.dirname(script_dir)  # Project root (parent of .ralph/)
    os.chdir(project_dir)  # Work from project root

    # Read prompt file
    try:
        prompt = get_prompt_content(args.prompt_file, script_dir)
    except FileNotFoundError as e:
        console = Console()
        console.print(f"[red]{e}[/red]")
        sys.exit(1)

    # Initialize state
    state = LoopState(
        engine=args.engine,
        mode=args.mode,
        iteration=0,
        max_iterations=args.max_iterations,
        prompt_file=args.prompt_file,
        script_dir=script_dir,
        project_dir=project_dir,
        cursor_model=args.cursor_model,
        debug_mode=args.debug,
        debug_log_path=os.path.join(script_dir, "session_log.log") if args.debug else None
    )

    if state.debug_mode and state.debug_log_path:
        try:
            with open(state.debug_log_path, "a", encoding="utf-8") as f:
                f.write("\n" + "=" * 80 + "\n")
                f.write(f"SESSION STARTED: {datetime.now()}\n")
                f.write("=" * 80 + "\n")
        except Exception as e:
            console = Console()
            console.print(f"[yellow]Warning: Could not create debug log: {e}[/yellow]")

    # Initial log entry
    state.add_log("🚀", "cyan", f"Ralph Loop started")
    state.add_log("⚙️", "blue", f"Engine: {args.engine}, Mode: {args.mode}")
    state.add_log("📄", "blue", f"Prompt: {args.prompt_file}")
    if args.max_iterations > 0:
        state.add_log("🔢", "blue", f"Max iterations: {args.max_iterations}")

    # Remove any stale stop signal from previous runs
    stop_file = Path(script_dir) / "STOP"
    if stop_file.exists():
        stop_file.unlink()
        state.add_log("🧹", "yellow", "Removed stale STOP file")

    console = Console()

    try:
        with Live(build_layout(state), console=console, refresh_per_second=4, screen=True) as live:
            while True:
                # Check for stop signal
                if stop_file.exists():
                    state.add_log("🛑", "green", "Stop signal detected - exiting loop")
                    live.update(build_layout(state))
                    stop_file.unlink()  # Clean up the stop file
                    break

                # Check max iterations
                if state.max_iterations > 0 and state.iteration >= state.max_iterations:
                    state.add_log("🏁", "yellow", f"Reached max iterations: {state.max_iterations}")
                    live.update(build_layout(state))
                    break

                state.iteration += 1

                # Run iteration
                success = run_iteration(state, prompt, live)

                if not success:
                    # Log error but continue to next iteration
                    state.add_log("⏭️", "yellow", "Continuing to next iteration despite error")
                    live.update(build_layout(state))

        # Final summary
        total_duration = state.get_total_duration()
        console.print()
        console.print(Panel(
            f"[bold]Ralph Loop finished after {state.iteration} iterations[/bold]\n"
            f"Total duration: {total_duration}\n"
            f"Total tokens - In: {state.input_tokens:,}, Out: {state.output_tokens:,}",
            title="[bold green]Complete[/bold green]",
            border_style="green"
        ))

    except KeyboardInterrupt:
        total_duration = state.get_total_duration()
        console.print()
        console.print(Panel(
            f"[bold]Loop interrupted at iteration {state.iteration}[/bold]\n"
            f"Total duration: {total_duration}",
            title="[bold yellow]Interrupted[/bold yellow]",
            border_style="yellow"
        ))
        sys.exit(130)


if __name__ == "__main__":
    main()
