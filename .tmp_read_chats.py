import json, glob, os

# Read VS Code chat editing sessions for Hollowreach workspace
base = "/home/b1s/.config/Code/User/workspaceStorage/ee30e4b803ea1ae242ee08d25de39fd6/chatEditingSessions"
sessions = []
for f in glob.glob(os.path.join(base, "*/state.json")):
    mtime = os.path.getmtime(f)
    sessions.append((mtime, f))
sessions.sort(reverse=True)

# Read latest 3 sessions
for mtime, path in sessions[:3]:
    print(f"\n{'='*60}")
    print(f"Session: {os.path.basename(os.path.dirname(path))}")
    print(f"Modified: {mtime}")
    with open(path) as fh:
        data = json.load(fh)
    tl = data.get("timeline", {})
    entries = tl.get("entries", [])
    for e in entries:
        if not isinstance(e, dict):
            continue
        req = e.get("request", {})
        msg = req.get("message", "")
        if msg:
            print(f"  USER: {str(msg)[:500]}")
        # Response
        resp = e.get("response", {})
        if isinstance(resp, dict):
            val = resp.get("value", "")
            if val:
                print(f"  ASST: {str(val)[:300]}")

# Also read Claude Code latest session - grep for "gemma" or "llm" related user messages
print(f"\n{'='*60}")
print("Claude Code - latest session (user messages with LLM/gemma context)")
cc_path = "/home/b1s/.claude/projects/-home-b1s-dev-github-hollowreach/99e77f34-0194-4494-81b6-5bba9a860a50.jsonl"
with open(cc_path) as fh:
    for line in fh:
        try:
            data = json.loads(line.strip())
            msg = data.get("message", {})
            role = msg.get("role", "")
            if role != "user":
                continue
            content = msg.get("content", [])
            for c in content:
                if isinstance(c, dict) and c.get("type") == "text":
                    text = c["text"]
                    if len(text) > 10:
                        # Print all user messages (they're short)
                        print(f"  USER [{data.get('timestamp','')}]: {text[:400]}")
        except:
            pass
