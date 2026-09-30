import json
import os

scenes_dir = os.path.expandvars(r"%APPDATA%\obs-studio\basic\scenes")
candidates = [os.path.join(scenes_dir, "Безымянный.json"), os.path.join(scenes_dir, "Untitled.json")]
path = next((p for p in candidates if os.path.exists(p)), candidates[0])
if not os.path.exists(path):
    print("Scene file not found in:", scenes_dir)
    exit(1)

with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)

modified = False
for s in data.get("sources", []):
    # Enable blewred AI filter
    if "filters" in s:
        for flt in s["filters"]:
            if flt.get("id") == "blewred_filter" or "blewred" in flt.get("name", "").lower():
                print(f"Enabling filter '{flt.get('name')}' on source '{s.get('name')}' (was {flt.get('enabled')})")
                flt["enabled"] = True
                modified = True
    
    # Disable blewred_Censor_Shield item in scene so it doesn't block video
    if s.get("id") == "scene" and "settings" in s and "items" in s["settings"]:
        for it in s["settings"]["items"]:
            if "blewred" in it.get("name", "").lower():
                print(f"Hiding scene item '{it.get('name')}' in scene '{s.get('name')}' (was {it.get('visible')})")
                it["visible"] = False
                modified = True

if modified:
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=4)
    print("Successfully updated OBS scene configuration!")
else:
    print("No changes were needed.")
