#!/usr/bin/env python3
"""Usage: campaign_phase_sweep.py REF_ID SHOT_SCREEN MENU_AT PHASES_JSON [HOVER_CLIP]
(needs SC_INSTALL and a release sc_view).

Coordinate-descent sweep of the campaign clip phases per reference.

For every clip of a screen (three disks, three portraits, the hovered On
clip) sweep its phase over one full loop in 1/10.5 s steps, scored on the
slot region, keep the best, and repeat once.  Prints the PHASES entry."""
import sys, os, subprocess, json, numpy as np
from concurrent.futures import ThreadPoolExecutor
from PIL import Image
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
R=ROOT/"engine-rust/docs/original_screenshots"
BIN=str(ROOT/"engine-rust/target/release/sc_view")
WORK=Path(os.environ.get("GC_SWEEP_DIR","/tmp/gc-phase-sweep")); WORK.mkdir(parents=True,exist_ok=True)
INST=os.environ["SC_INSTALL"]
DUR={"disk":2.4,"portrait":5.067,"terron":1.4,"proton":2.067,"on":1.334}
STEP=1/10.5
def loop(clip):
    if clip.startswith("disk"): d=DUR["disk"]
    elif clip in ("terron",): d=DUR["terron"]
    elif clip=="proton": d=DUR["proton"]
    elif clip.endswith("on"): d=DUR["on"]
    else: d=DUR["portrait"]
    return d*15/10.5
ORIG=[("zerg",(5,41,220,330)),("terr",(162,4,356,278)),("prot",(391,7,238,364))]
EXP=[("xterr",(5,41,240,330)),("xprot",(220,4,356,297)),("xzerg",(391,7,238,364))]
def region(r):
    x,y,w,h=r; return (int(320+3*x),int(3*y),int(320+3*(x+w)),min(1440,int(3*(y+h))))
def render(screen,at,phases,out):
    ph=",".join(f"{k}={v:.4f}" for k,v in phases.items())
    cmd=[BIN,"--install",INST,"--shot",out,"--shot-screen",screen,"--no-intro","--width","2560","--height","1440","--menu-at",f"{at:.4f}"]
    if ph: cmd+=["--menu-phase",ph]
    subprocess.run(cmd,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    a=np.asarray(Image.open(out).convert("RGB")).astype(np.int16)
    if "_final" not in out:
        os.remove(out)
    return a
def main(rid,screen,at,phases_json,hover):
    ref=np.asarray(Image.open(next(R.glob(f"Screenshot_*{rid}.png"))).convert("RGB").crop((440,0,3000,1440))).astype(np.int16)
    slots=EXP if "expansion" in screen else ORIG
    phases=json.loads(phases_json)
    clips=[]
    for name,r in slots:
        clips.append(("disk"+name,region(r)))
        clips.append((name,region(r)))
    if hover:
        r=dict(slots)[hover]; clips.append((hover+"on",region(r)))
    for c,_ in clips:
        phases.setdefault(c,at)
    def score(phases,box,tag):
        b=render(screen,at,phases,str(WORK/f"{rid}_{tag}.bmp"))
        x0,y0,x1,y1=box
        return float(np.abs(ref[y0:y1,x0:x1]-b[y0:y1,x0:x1]).mean())
    for rnd in range(2):
        for clip,box in clips:
            n=int(round(loop(clip)/STEP))
            cands=[i*STEP for i in range(n)]
            def ev(i_t):
                i,t=i_t; ph=dict(phases); ph[clip]=t; return (score(ph,box,f"{clip}_{i}"),t)
            with ThreadPoolExecutor(6) as ex:
                res=list(ex.map(ev,[(i,t) for i,t in enumerate(cands)]))
            best=min(res); phases[clip]=round(best[1],4)
            print(rid,rnd,clip,"best",round(best[0],3),"at",phases[clip],flush=True)
    b=render(screen,at,phases,str(WORK/f"{rid}_final.bmp"))
    print("FINAL",rid,screen,json.dumps(phases),"full-mean",round(float(np.abs(ref-b).mean()),3),flush=True)
if __name__=="__main__":
    main(sys.argv[1],sys.argv[2],float(sys.argv[3]),sys.argv[4],sys.argv[5] if len(sys.argv)>5 and sys.argv[5] else None)
