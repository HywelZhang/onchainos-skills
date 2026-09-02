#!/usr/bin/env python3
"""audit-events.py — event usage audit for okx/onchainos-skills forks.

Parses the Event enum + parse arms in state_machine.rs, then counts references
(snake_case wire tokens in .rs/.md, PascalCase variants in .rs) across the repo.
Verdicts: DEAD (0 refs anywhere) / docs-only / rust-only / active.
Re-run after every upstream sync to catch retired events.

Usage: python scripts/audit-events.py [repo-root]
"""
import os, re, sys
from collections import Counter

def main():
    R = sys.argv[1] if len(sys.argv) > 1 else '.'
    sm = os.path.join(R, 'cli/src/commands/agent_commerce/task/common/state_machine.rs')
    txt = open(sm, encoding='utf-8').read()

    m = re.search(r'pub enum Event \{(.*?)\n\}', txt, re.S)
    variants = re.findall(r'^\s{4}([A-Z][A-Za-z0-9]+),', m.group(1), re.M)
    parse_region = txt[txt.find('impl Event'):]
    arms = re.findall(r'"([a-z0-9_]+)"\s*=>\s*(?:Ok\()?Event::([A-Za-z0-9]+)', parse_region)
    wire = {}
    for w, v in arms:
        wire.setdefault(v, []).append(w)
    canon = {v: (wire[v][0] if v in wire else None) for v in variants}

    snakes = sorted({canon[v] for v in variants if canon[v]}, key=len, reverse=True)
    rx_snake = re.compile(r'\b(' + '|'.join(map(re.escape, snakes)) + r')\b')
    rx_pascal = re.compile(r'\b(' + '|'.join(map(re.escape, sorted(variants, key=len, reverse=True))) + r')\b')

    def scan(exts, exclude_sub=None):
        files = []
        for dp, _, fn in os.walk(R):
            if '.git' in dp or 'node_modules' in dp:
                continue
            if exclude_sub and exclude_sub in dp:
                continue
            files += [os.path.join(dp, f) for f in fn if f.endswith(exts)]
        return files

    rs_files = [f for f in scan(('.rs',), exclude_sub='state_machine.rs')]
    doc_files = scan(('.md',))

    def count(files, rx):
        c = Counter()
        for f in files:
            try:
                t = open(f, encoding='utf-8', errors='ignore').read()
            except OSError:
                continue
            for mm in rx.finditer(t):
                c[mm.group(1)] += 1
        return c

    c_rs, c_rsp, c_doc = count(rs_files, rx_snake), count(rs_files, rx_pascal), count(doc_files, rx_snake)
    print(f"{'variant':26s} {'wire':26s} {'rs':>4s} {'rsP':>4s} {'doc':>5s}  verdict")
    n_dead = n_active = n_rust = n_doc = 0
    for v in variants:
        w = canon[v]
        if not w:
            print(f"{v:26s} {'(NO parse arm)':26s}")
            continue
        a, b, c = c_rs.get(w, 0), c_rsp.get(v, 0), c_doc.get(w, 0)
        if a + b + c == 0:
            ver, n_dead = 'DEAD', n_dead + 1
        elif a + b == 0:
            ver, n_doc = 'docs-only', n_doc + 1
        elif c == 0:
            ver, n_rust = 'rust-only', n_rust + 1
        else:
            ver, n_active = 'active', n_active + 1
        print(f"{v:26s} {w:26s} {a:4d} {b:4d} {c:5d}  {ver}")
    print(f"\nvariants={len(variants)} active={n_active} rust-only={n_rust} docs-only={n_doc} dead={n_dead}")

if __name__ == '__main__':
    main()
