# Scripted agent session (M8): drive quarry-mcp over stdio like Claude would.
# Usage: python3 agent_session.py <quarry-mcp binary> <empty work folder>
# Run through: cargo run -p xtask -- agent-session
import json, subprocess, sys, os, time, base64
exe, work = sys.argv[1], sys.argv[2]
env = dict(os.environ, QUARRY_SOCKET="q.sock", QUARRY_DATA_DIR=os.path.join(work, "data"), QUARRY_HEADLESS_IDLE_SECS="20")
p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=work, env=env, text=True)
n = [0]
def rpc(method, params):
    n[0] += 1; i = n[0]
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n"); p.stdin.flush()
    while True:
        line = json.loads(p.stdout.readline())
        if line.get("id") == i: return line
start = time.time()
rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "session", "version": "0"}})
p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
guide = rpc("resources/read", {"uri": "quarry://guide"})["result"]["contents"][0]["text"]
print("guide:", guide.splitlines()[0], f"({len(guide)} chars)")
def call(name, args, show=True):
    r = rpc("tools/call", {"name": name, "arguments": args})["result"]
    text = next((c["text"] for c in r["content"] if c["type"] == "text"), "")
    if r.get("isError"): print("ERROR", name, text); return r
    if show: print("ok", name, text[:90])
    return r
path = os.path.join(work, "crate.quarry")
steps = [
  ("project_new", {"name": "Crate", "path": path}),
  ("mesh_add_primitive", {"kind": "box", "name": "Crate", "position": [0, 0.5, 0]}),
  ("select_mode", {"mode": "face", "object": "Crate"}),
  ("paint_palette_lock", {"lock": False}),
  ("paint_texture_new", {"width": 32, "height": 32, "color": [150, 100, 60, 255], "name": "crate_tex"}),
  ("uv_unwrap", {}),
  ("paint_rect", {"image": "crate_tex", "x": 0, "y": 0, "width": 32, "height": 2, "rgba": [70, 45, 25, 255]}),
  ("rig_skeleton_new", {"name": "rig"}),
  ("rig_bone_add", {"name": "root", "head": [0, 0, 0], "tail": [0, 0.5, 0]}),
  ("rig_bone_add", {"name": "lid", "parent": "root", "tail": [0, 1, 0]}),
  ("rig_bind_auto", {"object": "Crate"}),
  ("anim_clip_new", {"name": "idle"}),
  ("anim_key_set", {"bone": "root", "channel": "rotation", "time": 0, "value": [0, 0, 0]}),
  ("anim_key_set", {"bone": "root", "channel": "rotation", "time": 0.5, "value": [0, 5, 0]}),
  ("anim_key_set", {"bone": "root", "channel": "rotation", "time": 1, "value": [0, 0, 0]}),
  ("anim_clip_new", {"name": "open", "length": 1}),
  ("anim_key_set", {"bone": "lid", "channel": "rotation", "time": 0, "value": [0, 0, 0], "interp": "ease_in_out"}),
  ("anim_key_set", {"bone": "lid", "channel": "rotation", "time": 1, "value": [-60, 0, 0]}),
]
for name, args in steps: call(name, args)
for preset in ["front", "side", "iso"]:
    r = call("view_screenshot", {"preset": preset}, show=False)
    img = next(c for c in r["content"] if c["type"] == "image")
    data = base64.b64decode(img["data"])
    open(os.path.join(work, f"shot-{preset}.png"), "wb").write(data)
    print("screenshot", preset, img["mimeType"], len(data), "bytes")
call("export_run", {"path": os.path.join(work, "crate.glb")})
call("project_save", {})
call("export_run", {"path": "/tmp/outside.glb"})
call("query_history", {"n": 3})
print(f"session took {time.time() - start:.1f} s")
p.stdin.close(); p.wait(timeout=10)
assert os.path.isfile(os.path.join(work, "crate.glb")), "the prop was exported"
