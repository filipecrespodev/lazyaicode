#!/usr/bin/env python3
"""Lista as chaves de tradução usadas no código Rust da UI que faltam em
crates/i18n/locales/en.json (ou em qualquer outro idioma)."""
import json, os, re, sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
loc = {l: json.load(open(os.path.join(ROOT, f"crates/i18n/locales/{l}.json"))) for l in ["en", "pt", "es", "zh"]}
used = set()
pat = re.compile(r'\b(?:t|tf|label_of|lookup)\(\s*"([a-zA-Z][\w]*\.[\w.]+)"')
lab = re.compile(r'"((?:[a-z][a-zA-Z]+)\.(?:[a-zA-Z][\w]*))"')
for d in ["crates/ui-egui/src", "crates/doc/src"]:
    for dp, _, fs in os.walk(os.path.join(ROOT, d)):
        for f in fs:
            if f.endswith(".rs"):
                s = open(os.path.join(dp, f)).read()
                used |= set(pat.findall(s))
                # rótulos de comandos e tabelas (strings no formato área.chave)
                for k in lab.findall(s):
                    ns = k.split(".")[0]
                    if ns in {"ui", "git", "thread", "engine", "settings", "prefs", "account", "workspace", "project", "capabilities", "provider", "system", "terminal", "review", "github", "gitlab", "ai", "wsl", "events", "image", "bytes", "cmd", "egui", "lazyaicode", "std", "serde", "crate", "super", "self", "json", "p", "s", "c", "r", "e", "a", "v", "i", "x", "data", "ui-egui"}:
                        if k.split(".")[0] in {"thread", "settings"} and k in loc["en"]:
                            used.add(k)
                        continue
                    used.add(k)
# formatos montados: settings.tab.<id>, agentReview.base.<id>, thread.scope<X>, permission.<id>
for tab in ["workspaces","appearance","skills","agents","mcp","accounts","providers","defaults","git","integrations","shortcuts","about"]:
    used.add(f"settings.tab.{tab}")
for b in ["upstream","turn","head"]:
    used.add(f"agentReview.base.{b}")
for sc in ["Project","Global","Plugin","Other","Builtin"]:
    used.add(f"thread.scope{sc}")
for p in ["allow","deny"]:
    used.add(f"permission.{p}")
missing = {l: sorted(k for k in used if k not in d) for l, d in loc.items()}
if "--json" in sys.argv:
    print(json.dumps(missing["en"]))
else:
    for k in missing["en"]:
        print(k)
    print(f"# {len(missing['en'])} faltando em en; pt {len(missing['pt'])}, es {len(missing['es'])}, zh {len(missing['zh'])}", file=sys.stderr)
