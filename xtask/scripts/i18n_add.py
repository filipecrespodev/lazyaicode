#!/usr/bin/env python3
"""Acrescenta chaves de tradução aos quatro locales (sem sobrescrever as que
já existem). Uso: python3 xtask/scripts/i18n_add.py <arquivo.py com T = {chave: (en, pt, es, zh)}>"""
import json, os, sys, runpy
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
T = runpy.run_path(sys.argv[1])["T"]
for i, l in enumerate(["en", "pt", "es", "zh"]):
    p = os.path.join(ROOT, f"crates/i18n/locales/{l}.json")
    d = json.load(open(p))
    added = 0
    for k, v in T.items():
        if k not in d or "--force" in sys.argv:
            d[k] = v[i]
            added += 1
    json.dump(dict(sorted(d.items())), open(p, "w"), ensure_ascii=False, indent=1)
    open(p, "a").write("\n")
    print(l, "+", added)
