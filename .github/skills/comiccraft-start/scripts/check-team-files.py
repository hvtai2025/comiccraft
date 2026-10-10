#!/usr/bin/env python3
"""Validate the ComicCraft agent-customization files.

Checks every file under .github/agents, .github/instructions, .github/prompts and
.github/skills for the mistakes that fail silently in VS Code: a missing or vague
description, a skill whose `name` does not match its folder, an `agents:` or `agent:`
reference that does not resolve to a real agent file, a tool alias that does not exist,
and an instruction with no applyTo and no usable description.

Deliberately dependency-free: PyYAML is used when available, otherwise a small parser
handles the flat frontmatter these files use. Exit code is 1 when anything FAILs.

Usage: python3 check-team-files.py [repo-root]
"""

import os
import re
import sys

TOOL_ALIASES = {"execute", "read", "edit", "search", "agent", "web", "todo"}
KNOWN_AGENT_KEYS = {
    "description", "name", "tools", "model", "reasoning-effort", "argument-hint",
    "agents", "user-invocable", "disable-model-invocation", "handoffs", "hooks", "target",
}
KNOWN_INSTRUCTION_KEYS = {"description", "name", "applyTo"}
KNOWN_PROMPT_KEYS = {"description", "name", "agent", "model", "tools", "argument-hint"}
KNOWN_SKILL_KEYS = {
    "name", "description", "argument-hint", "user-invocable", "disable-model-invocation",
    "license", "allowed-tools", "metadata",
}


def parse_frontmatter(path):
    """Return (mapping, body, error)."""
    try:
        text = open(path, encoding="utf-8").read()
    except OSError as exc:
        return None, "", f"unreadable: {exc}"
    if not text.startswith("---"):
        return None, text, "no YAML frontmatter (the file must start with ---)"
    end = text.find("\n---", 3)
    if end == -1:
        return None, text, "frontmatter is not closed with ---"
    raw = text[3:end].strip("\n")
    body = text[end + 4:]

    try:
        import yaml  # type: ignore
    except ImportError:
        data = {}
        for line in raw.splitlines():
            line = line.split(" #")[0].rstrip()
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            m = re.match(r"^([A-Za-z][\w-]*):\s*(.*)$", line)
            if not m:
                return None, body, f"cannot parse frontmatter line: {line!r}"
            key, value = m.group(1), m.group(2).strip()
            if value.startswith("[") and value.endswith("]"):
                inner = value[1:-1].strip()
                data[key] = [v.strip().strip("\"'") for v in inner.split(",") if v.strip()] if inner else []
            else:
                data[key] = value.strip("\"'")
        return data, body, None

    try:
        data = yaml.safe_load(raw) or {}
    except Exception as exc:  # noqa: BLE001 - report, never crash
        return None, body, f"invalid YAML: {exc}"
    if not isinstance(data, dict):
        return None, body, "frontmatter is not a mapping"
    return data, body, None


def check_list(data, key, path, problems):
    value = data.get(key)
    if value is None:
        return None
    if isinstance(value, str):
        problems.append(f"{path}: `{key}` must be a YAML list, not a string")
        return None
    if not isinstance(value, list):
        problems.append(f"{path}: `{key}` must be a YAML list")
        return None
    return [str(v) for v in value]


def check_unknown_keys(data, known, path, problems):
    for key in data:
        if key not in known:
            problems.append(f"{path}: unknown frontmatter key `{key}` (typo?)")


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    gh = os.path.join(root, ".github")
    if not os.path.isdir(gh):
        print(f"FAIL .github not found under {os.path.abspath(root)}")
        return 1

    problems, notes = [], []

    def listing(sub, suffix):
        d = os.path.join(gh, sub)
        return sorted(f for f in os.listdir(d) if f.endswith(suffix)) if os.path.isdir(d) else []

    def fm(rel):
        return parse_frontmatter(os.path.join(gh, rel))

    agents = [f[: -len(".agent.md")] for f in listing("agents", ".agent.md")]
    if not agents:
        problems.append(".github/agents: no *.agent.md files found")

    for name in agents:
        rel = f"agents/{name}.agent.md"
        data, body, err = fm(rel)
        if err:
            problems.append(f".github/{rel}: {err}")
            continue
        path = f".github/{rel}"
        check_unknown_keys(data, KNOWN_AGENT_KEYS, path, problems)
        if not str(data.get("description", "")).strip():
            problems.append(f"{path}: missing `description`")
        if data.get("name"):
            problems.append(
                f"{path}: remove `name` ({data['name']!r}) — the file name must stay the agent "
                "identifier, or `agents:`/`agent:` references break"
            )
        tools = check_list(data, "tools", path, problems)
        if tools is not None:
            unknown = [t for t in tools if t not in TOOL_ALIASES]
            if unknown:
                problems.append(f"{path}: unknown tool alias(es) {unknown}; known: {sorted(TOOL_ALIASES)}")
        subs = check_list(data, "agents", path, problems)
        if subs is not None:
            missing = [s for s in subs if s not in agents]
            if missing:
                problems.append(f"{path}: `agents` references unknown agent(s) {missing}")
            if tools is not None and "agent" not in tools:
                problems.append(f"{path}: lists `agents` but does not include the `agent` tool")
        if not body.strip():
            problems.append(f"{path}: empty body (no instructions)")

    for filename in listing("instructions", ".instructions.md"):
        rel = f"instructions/{filename}"
        data, body, err = fm(rel)
        path = f".github/{rel}"
        if err:
            problems.append(f"{path}: {err}")
            continue
        check_unknown_keys(data, KNOWN_INSTRUCTION_KEYS, path, problems)
        desc = str(data.get("description", "")).strip()
        if not desc:
            problems.append(f"{path}: missing `description`")
        if not data.get("applyTo") and "use when" not in desc.lower():
            problems.append(f"{path}: neither `applyTo` nor a 'Use when…' description, so it can never be discovered")
        if data.get("applyTo") == "**":
            notes.append(f"{path}: applyTo is '**' — loaded for every request; confirm that is intended")
        if not body.strip():
            problems.append(f"{path}: empty body")

    for filename in listing("prompts", ".prompt.md"):
        rel = f"prompts/{filename}"
        data, body, err = fm(rel)
        path = f".github/{rel}"
        if err:
            problems.append(f"{path}: {err}")
            continue
        check_unknown_keys(data, KNOWN_PROMPT_KEYS, path, problems)
        if not str(data.get("description", "")).strip():
            problems.append(f"{path}: missing `description`")
        agent = data.get("agent")
        if agent and str(agent) not in agents:
            problems.append(f"{path}: `agent: {agent}` does not resolve to an agent file")
        if not body.strip():
            problems.append(f"{path}: empty body")

    skills_dir = os.path.join(gh, "skills")
    skills = []
    if os.path.isdir(skills_dir):
        for folder in sorted(os.listdir(skills_dir)):
            rel = f"skills/{folder}/SKILL.md"
            path = f".github/{rel}"
            if not os.path.isfile(os.path.join(gh, rel)):
                problems.append(f"{path}: missing SKILL.md")
                continue
            skills.append(folder)
            data, body, err = fm(rel)
            if err:
                problems.append(f"{path}: {err}")
                continue
            check_unknown_keys(data, KNOWN_SKILL_KEYS, path, problems)
            name = str(data.get("name", "")).strip()
            if not name:
                problems.append(f"{path}: missing `name`")
            elif name != folder:
                problems.append(f"{path}: `name: {name}` must match the folder name `{folder}`")
            elif not re.fullmatch(r"[a-z0-9-]{1,64}", name):
                problems.append(f"{path}: `name` must be lowercase alphanumeric or hyphens, 1-64 chars")
            desc = str(data.get("description", "")).strip()
            if not desc:
                problems.append(f"{path}: missing `description`")
            elif len(desc) > 1024:
                problems.append(f"{path}: `description` is {len(desc)} chars, over the 1024 limit")
            if not body.strip():
                problems.append(f"{path}: empty body (no procedure)")
            for ref in re.findall(r"\]\((\./[^)\s]+)\)", body):
                if not os.path.exists(os.path.normpath(os.path.join(gh, "skills", folder, ref))):
                    problems.append(f"{path}: references {ref}, which does not exist")
            scripts = os.path.join(skills_dir, folder, "scripts")
            if os.path.isdir(scripts):
                for script in sorted(os.listdir(scripts)):
                    full = os.path.join(scripts, script)
                    if script.endswith((".sh", ".py")) and not os.access(full, os.X_OK):
                        notes.append(f".github/skills/{folder}/scripts/{script}: not executable (chmod +x)")

    print("ComicCraft agent-customization check")
    print(f"  agents: {len(agents)}  skills: {len(skills)}")
    for note in notes:
        print(f"  note: {note}")
    if problems:
        print(f"\n{len(problems)} problem(s):")
        for p in problems:
            print(f"  FAIL {p}")
        return 1
    print("\nAll files valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
