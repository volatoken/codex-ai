"""Quick test: poll Telegram, call adapter, send reply. Tests the full chain."""
import os, sys, json, time
import httpx

# Load .env
env_path = os.path.join(os.path.dirname(__file__), ".env")
env = {}
with open(env_path) as f:
    for line in f:
        line = line.strip()
        if "=" in line and not line.startswith("#"):
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip()

BOT_TOKEN = env["TELEGRAM_BOT_TOKEN"]
GROUP_ID = int(env["TELEGRAM_GROUP_ID"])
ADAPTER_URL = "http://localhost:2024"
TG_API = f"https://api.telegram.org/bot{BOT_TOKEN}"

def get_updates(offset=None, timeout=10):
    params = {"timeout": timeout, "allowed_updates": json.dumps(["message"])}
    if offset:
        params["offset"] = offset
    r = httpx.get(f"{TG_API}/getUpdates", params=params, timeout=30)
    return r.json()

def send_message(chat_id, text, thread_id=None):
    data = {"chat_id": chat_id, "text": text[:4096]}
    if thread_id:
        data["message_thread_id"] = thread_id
    r = httpx.post(f"{TG_API}/sendMessage", json=data, timeout=30)
    return r.json()

def call_adapter(user_msg):
    # Create thread
    r = httpx.post(f"{ADAPTER_URL}/api/chat/thread", json={}, timeout=10)
    tid = r.json()["thread_id"]
    # Call fast chat
    body = {"messages": [{"role": "user", "content": user_msg}], "thread_id": tid}
    r = httpx.post(f"{ADAPTER_URL}/api/chat/fast", json=body, timeout=120)
    # Parse SSE
    answer = []
    for line in r.text.split("\n"):
        if line.startswith("data: "):
            data = line[6:]
            if data.strip() == "[DONE]":
                break
            try:
                evt = json.loads(data)
                c = evt.get("content", "")
                if c:
                    answer.append(c)
            except:
                pass
    return "".join(answer)

print(f"Bot token: {BOT_TOKEN[:15]}...")
print(f"Group ID: {GROUP_ID}")
print(f"Adapter: {ADAPTER_URL}")
print("Polling for messages (send a message in the group)...")

offset = None
while True:
    try:
        result = get_updates(offset, timeout=10)
        if not result.get("ok"):
            print(f"getUpdates error: {result}")
            time.sleep(5)
            continue
        updates = result.get("result", [])
        for upd in updates:
            offset = upd["update_id"] + 1
            msg = upd.get("message", {})
            chat_id = msg.get("chat", {}).get("id")
            text = msg.get("text", "")
            user = msg.get("from", {}).get("first_name", "?")
            thread_id = msg.get("message_thread_id")
            
            print(f"\n[MSG] chat={chat_id} thread={thread_id} user={user}: {text[:80]}")
            
            if chat_id != GROUP_ID:
                print(f"  -> Skipping (not our group)")
                continue
            if not text:
                continue
                
            print(f"  -> Calling adapter...")
            try:
                reply = call_adapter(text)
                print(f"  -> Got reply ({len(reply)} chars): {reply[:100]}...")
                
                resp = send_message(chat_id, reply, thread_id)
                if resp.get("ok"):
                    print(f"  -> Sent to Telegram OK!")
                else:
                    print(f"  -> Telegram error: {resp}")
            except Exception as e:
                print(f"  -> Error: {e}")
                send_message(chat_id, f"Error: {e}", thread_id)
    except KeyboardInterrupt:
        print("\nStopping...")
        break
    except Exception as e:
        print(f"Poll error: {e}")
        time.sleep(5)
