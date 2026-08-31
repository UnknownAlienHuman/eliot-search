#!/usr/bin/env python3
from __future__ import annotations
import json, tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
 "search-config": ("crates/search-config", ["search-contracts"], ["C0","C1","C2","C3"]),
 "search-runtime-owner": ("crates/search-runtime/search-runtime-owner", ["search-contracts","search-domain","search-ports","search-config"], ["R0","R1","R2","R3"]),
 "search-os-secrets": ("crates/search-runtime/search-os-secrets", ["search-contracts","search-domain","search-ports","search-config"], ["S0","S1","S2","S3"]),
 "search-control-redb": ("crates/search-control-redb", ["search-contracts","search-domain","search-ports","search-config"], ["J0","J1","J2","J3"]),
 "search-provider-protocol": ("crates/search-provider-protocol", ["search-contracts","search-domain","search-ports","search-config"], ["P0","P1","P2","P3"]),
 "eliot-searchd": ("bins/eliot-searchd", ["search-contracts","search-domain","search-ports","search-config","search-runtime-owner","search-os-secrets","search-control-redb","search-provider-protocol"], ["D0","D1","D2","D3"]),
 "eliot-search": ("bins/eliot-search", ["search-contracts","search-ports","search-config","search-provider-protocol"], ["L0","L1","L2","L3"]),
}

def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))

def map_rows(doc: dict[str, Any], key: str) -> dict[str, dict[str, Any]]:
    rows=doc.get(key)
    if not isinstance(rows,list): raise ValueError(f"{key} must be array")
    out={}
    for row in rows:
        if not isinstance(row,dict) or not isinstance(row.get("name"),str): raise ValueError(f"bad {key} row")
        if row["name"] in out: raise ValueError(f"duplicate {row['name']}")
        out[row["name"]]=row
    return out

def main() -> int:
    errors=[]
    try:
        doc=load("swarm/w1-milestone-packets.toml")
        agent=map_rows(load("swarm/w1-agent-packets.toml"),"package")
        launch=load("swarm/launch-state.toml")
        cases=load("qualification/w1-milestones/cases-v1.toml")
        rows=map_rows(doc,"package")
    except (OSError,UnicodeDecodeError,tomllib.TOMLDecodeError,ValueError) as exc:
        print(json.dumps({"status":"FAIL","errors":[str(exc)]},indent=2)); return 1
    if set(rows)!=set(EXPECTED): errors.append("package set mismatch")
    if doc.get("package_count")!=7 or doc.get("milestone_count")!=28: errors.append("count mismatch")
    if doc.get("status")!="BLOCKED_ON_G0_AND_W0": errors.append("registry not blocked")
    if doc.get("requires_accepted_gates")!=["G0"] or doc.get("requires_accepted_receipts")!=["W0"]: errors.append("prerequisite mismatch")
    if doc.get("one_writer_one_package") is not True or doc.get("sequential_milestones_per_package") is not True: errors.append("ownership/order disabled")
    if doc.get("parallel_milestones_within_package") is not False or doc.get("implementation_authorized_by_this_registry") is not False: errors.append("authority ceiling failed")
    if launch.get("active_stage")!="P00" or launch.get("active_wave")!=0 or launch.get("authorized_packages")!=["search-contracts"]: errors.append("launch moved")
    for name,(path,deps,mids) in EXPECTED.items():
        row=rows.get(name,{})
        ar=agent.get(name,{})
        if row.get("path")!=path or row.get("write_scope")!=path+"/**": errors.append(f"{name}: scope")
        if row.get("required_handoff_packages")!=deps or ar.get("required_handoff_packages")!=deps: errors.append(f"{name}: deps")
        if row.get("milestone_ids")!=mids or row.get("one_active_milestone") is not True or row.get("claimable") is not False: errors.append(f"{name}: milestones")
        packet=row.get("packet")
        if not isinstance(packet,str) or not (ROOT/packet).is_file(): errors.append(f"{name}: packet missing")
        else:
            text=(ROOT/packet).read_text(encoding="utf-8")
            for mid in mids:
                if f"## {mid} —" not in text: errors.append(f"{name}: missing {mid}")
            if "docs/architecture/" in text or "/src/" in text: errors.append(f"{name}: forbidden read")
    case_rows=cases.get("case")
    if cases.get("case_count")!=16 or not isinstance(case_rows,list) or len(case_rows)!=16: errors.append("case inventory")
    elif any(x.get("mandatory") is not True or x.get("result")!="UNAVAILABLE" for x in case_rows): errors.append("case state")
    workflow=ROOT/".github/workflows/w1-milestone-packets.yml"
    if not workflow.is_file(): errors.append("workflow missing")
    else:
        text=workflow.read_text(encoding="utf-8")
        for token in ("workflow_dispatch:","contents: read","persist-credentials: false"):
            if token not in text: errors.append(f"workflow missing {token}")
        for token in ("\n  push:","\n  pull_request:","\n  schedule:","\n  workflow_run:"):
            if token in text: errors.append(f"automatic trigger {token.strip()}")
    result={"status":"PASS" if not errors else "FAIL","packages":len(rows),"milestones":sum(len(x[2]) for x in EXPECTED.values()),"cases":len(case_rows) if isinstance(case_rows,list) else 0,"launch_stage":launch.get("active_stage"),"launch_wave":launch.get("active_wave"),"errors":errors}
    print(json.dumps(result,indent=2,sort_keys=True)); return 0 if not errors else 1
if __name__=="__main__": raise SystemExit(main())
